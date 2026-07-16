//! Parallel encoding orchestration.
//!
//! Splits a transcoding job into chunks and processes them in parallel,
//! then stitches the results together. This is the key feature for
//! high-throughput media server workflows.

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::chunking::{Chunk, ChunkPlan};
use crate::scheduler::SchedulerPolicy;
use crate::transcode::{TranscodeObserver, TranscodeProgress, TranscodeStatus};

/// A work unit for parallel processing.
#[derive(Debug, Clone)]
pub struct ChunkJob {
    pub chunk: Chunk,
    pub input_path: String,
    pub output_path: String,
    pub qp: i8,
}

/// Result of encoding a single chunk.
#[derive(Debug)]
pub struct ChunkResult {
    pub chunk_index: usize,
    pub frames_encoded: u64,
    pub bytes_written: u64,
    pub elapsed: Duration,
}

/// Parallel encoding engine.
///
/// Takes a ChunkPlan and distributes work across a thread pool.
/// Each thread encodes its assigned chunk independently.
pub struct ParallelEncoder {
    policy: SchedulerPolicy,
}

impl ParallelEncoder {
    pub fn new(policy: SchedulerPolicy) -> Self {
        Self { policy }
    }

    /// Encode chunks in parallel.
    ///
    /// `jobs` - one ChunkJob per chunk
    /// `encoder_fn` - function that encodes a chunk, returning ChunkResult
    /// `observer` - optional progress callback
    ///
    /// Returns results in chunk order.
    pub fn encode_parallel<F>(
        &self,
        jobs: Vec<ChunkJob>,
        encoder_fn: F,
        observer: Option<&dyn TranscodeObserver>,
    ) -> Vec<ChunkResult>
    where
        F: Fn(&ChunkJob) -> ChunkResult + Send + Sync + 'static,
    {
        let num_chunks = jobs.len();
        let workers = self.policy.effective_workers().min(num_chunks);
        let encoder_fn = std::sync::Arc::new(encoder_fn);

        let (tx, rx) = mpsc::channel::<ChunkResult>();
        let mut handles = Vec::with_capacity(workers);

        // Distribute jobs across workers
        let jobs_per_worker = (num_chunks + workers - 1) / workers;

        for worker_id in 0..workers {
            let start_idx = worker_id * jobs_per_worker;
            let end_idx = (start_idx + jobs_per_worker).min(num_chunks);
            if start_idx >= num_chunks {
                break;
            }

            let worker_jobs: Vec<ChunkJob> = jobs[start_idx..end_idx].to_vec();
            let tx = tx.clone();
            let encoder_fn = encoder_fn.clone();

            let handle = thread::spawn(move || {
                for job in &worker_jobs {
                    let result = encoder_fn(job);
                    let _ = tx.send(result);
                }
            });
            handles.push(handle);
        }

        drop(tx); // Close sender so rx.recv() returns None when all done

        // Collect results
        let mut results: Vec<Option<ChunkResult>> = (0..num_chunks).map(|_| None).collect();
        let mut completed = 0;

        while let Ok(result) = rx.recv() {
            let idx = result.chunk_index;
            results[idx] = Some(result);
            completed += 1;

            if let Some(obs) = observer {
                obs.on_progress(&TranscodeProgress {
                    status: TranscodeStatus::Encoding,
                    frames_encoded: completed as u64,
                    total_frames: Some(num_chunks as u64),
                    elapsed: Duration::ZERO,
                    current_fps: 0.0,
                    bytes_written: 0,
                });
            }
        }

        // Wait for all workers to finish
        for handle in handles {
            let _ = handle.join();
        }

        // Return results in chunk order
        results.into_iter().map(|r| r.unwrap()).collect()
    }
}

/// Estimate the optimal number of chunks for a given file.
///
/// Considers file duration, available workers, and minimum chunk size.
pub fn estimate_chunk_count(
    total_frames: u64,
    fps: f64,
    max_workers: usize,
    min_chunk_seconds: f64,
) -> usize {
    let total_seconds = total_frames as f64 / fps;
    let max_chunks_by_duration = (total_seconds / min_chunk_seconds).ceil() as usize;
    let max_chunks = max_workers.max(1);
    max_chunks_by_duration.min(max_chunks).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_encoder_single_chunk() {
        let policy = SchedulerPolicy {
            max_simultaneous_jobs: 1,
            cpu_workers: 1,
            chunked_encoding: true,
            max_chunks_per_file: 1,
        };
        let encoder = ParallelEncoder::new(policy);
        let jobs = vec![ChunkJob {
            chunk: Chunk {
                index: 0,
                start_frame: 0,
                end_frame: 100,
                start_pts: Duration::ZERO,
                end_pts: Duration::from_secs_f64(100.0 / 24.0),
            },
            input_path: "test.mkv".into(),
            output_path: "out.mkv".into(),
            qp: 26,
        }];

        let results = encoder.encode_parallel(
            jobs,
            |job| ChunkResult {
                chunk_index: job.chunk.index,
                frames_encoded: job.chunk.frame_count(),
                bytes_written: 1000,
                elapsed: Duration::from_millis(100),
            },
            None,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].frames_encoded, 100);
    }

    #[test]
    fn parallel_encoder_multiple_chunks() {
        let policy = SchedulerPolicy {
            max_simultaneous_jobs: 4,
            cpu_workers: 4,
            chunked_encoding: true,
            max_chunks_per_file: 4,
        };
        let encoder = ParallelEncoder::new(policy);
        let jobs: Vec<ChunkJob> = (0..4u64)
            .map(|i| ChunkJob {
                chunk: Chunk {
                    index: i as usize,
                    start_frame: i * 25,
                    end_frame: (i + 1) * 25,
                    start_pts: Duration::from_secs_f64(i as f64 * 25.0 / 24.0),
                    end_pts: Duration::from_secs_f64((i + 1) as f64 * 25.0 / 24.0),
                },
                input_path: "test.mkv".into(),
                output_path: format!("out_{i}.mkv"),
                qp: 26,
            })
            .collect();

        let results = encoder.encode_parallel(
            jobs,
            |job| ChunkResult {
                chunk_index: job.chunk.index,
                frames_encoded: job.chunk.frame_count(),
                bytes_written: 500,
                elapsed: Duration::from_millis(50),
            },
            None,
        );

        assert_eq!(results.len(), 4);
        // Results should be in chunk order
        for (i, result) in results.iter().enumerate() {
            assert_eq!(result.chunk_index, i);
        }
    }

    #[test]
    fn estimate_chunk_count_short_video() {
        // 10 seconds at 24fps, min 5s chunks → 2 chunks
        assert_eq!(estimate_chunk_count(240, 24.0, 4, 5.0), 2);
    }

    #[test]
    fn estimate_chunk_count_long_video() {
        // 100 seconds at 24fps, min 10s chunks, 4 workers → 4 chunks
        assert_eq!(estimate_chunk_count(2400, 24.0, 4, 10.0), 4);
    }

    #[test]
    fn estimate_chunk_count_limits_to_workers() {
        // 1000 seconds at 24fps, min 1s chunks, 4 workers → 4 chunks (limited by workers)
        assert_eq!(estimate_chunk_count(24000, 24.0, 4, 1.0), 4);
    }

    #[test]
    fn estimate_chunk_count_minimum_one() {
        // Very short video → at least 1 chunk
        assert_eq!(estimate_chunk_count(10, 24.0, 4, 100.0), 1);
    }
}
