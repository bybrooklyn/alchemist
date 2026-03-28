# OpenAPI Specification

Full OpenAPI source for the Alchemist API.

```yaml
openapi: 3.0.3
info:
  title: Alchemist API
  description: |
    REST API for Alchemist - a self-hosted media transcoding pipeline.
    
    ## Authentication
    
    Most endpoints require authentication. Obtain a session token via `/auth/login` and then include it as:
    - **Cookie**: `alchemist_session=<token>`
    - **Header**: `Authorization: Bearer <token>`
    
    ## Rate Limiting
    
    - **Login attempts**: 10 per second
    - **Global API**: 120 requests per minute
    
    ## Versioning
    
    The API is available at both `/api/v1/*` (versioned) and `/api/*` (backwards-compatible alias).
  version: 1.0.0
  contact:
    name: Alchemist
    url: https://github.com/alchemist-dev/alchemist
  license:
    name: MIT

servers:
  - url: /api/v1
    description: Versioned API (recommended)
  - url: /api
    description: Backwards-compatible alias

tags:
  - name: Auth
    description: Authentication and session management
  - name: Jobs
    description: Transcoding job management
  - name: Stats
    description: Statistics and metrics
  - name: Logs
    description: System and job logging
  - name: Events
    description: Server-sent events for real-time updates
  - name: Engine
    description: Transcoding engine control
  - name: Settings
    description: Configuration management
  - name: Scan
    description: Library scanning operations
  - name: Profiles
    description: Encoding profile management
  - name: Library
    description: Library health and management
  - name: System
    description: System information and health checks
  - name: Filesystem
    description: Filesystem browsing
  - name: Setup
    description: Initial setup wizard

paths:
  # ============================================================================
  # AUTH ENDPOINTS
  # ============================================================================
  /auth/login:
    post:
      tags: [Auth]
      summary: Authenticate user
      description: Login with username and password to obtain a session token.
      operationId: login
      security: []
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/LoginRequest'
      responses:
        '200':
          description: Login successful
          headers:
            Set-Cookie:
              schema:
                type: string
                example: alchemist_session=abc123; HttpOnly; SameSite=Lax; Max-Age=2592000
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/StatusResponse'
        '401':
          description: Invalid credentials
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ErrorResponse'
        '429':
          description: Too many login attempts
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ErrorResponse'

  /auth/logout:
    post:
      tags: [Auth]
      summary: Logout and invalidate session
      operationId: logout
      responses:
        '200':
          description: Logout successful
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/StatusResponse'

  # ============================================================================
  # JOBS ENDPOINTS
  # ============================================================================
  /jobs:
    get:
      tags: [Jobs]
      summary: List jobs with filtering and pagination
      operationId: listJobs
      parameters:
        - name: limit
          in: query
          schema:
            type: integer
            minimum: 1
            maximum: 200
            default: 50
        - name: page
          in: query
          schema:
            type: integer
            minimum: 1
            default: 1
        - name: status
          in: query
          description: Comma-separated list of statuses
          schema:
            type: string
            example: Queued,Encoding,Completed
        - name: search
          in: query
          description: Search in job paths/names
          schema:
            type: string
        - name: sort_by
          in: query
          schema:
            type: string
        - name: sort_desc
          in: query
          schema:
            type: boolean
            default: false
        - name: archived
          in: query
          schema:
            type: string
            enum: ['true', 'false']
            default: 'false'
      responses:
        '200':
          description: List of jobs
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/Job'

  /jobs/table:
    get:
      tags: [Jobs]
      summary: List jobs (alias for /jobs)
      operationId: listJobsTable
      parameters:
        - $ref: '#/components/parameters/LimitParam'
        - $ref: '#/components/parameters/PageParam'
        - name: status
          in: query
          schema:
            type: string
        - name: search
          in: query
          schema:
            type: string
        - name: sort_by
          in: query
          schema:
            type: string
        - name: sort_desc
          in: query
          schema:
            type: boolean
        - name: archived
          in: query
          schema:
            type: string
            enum: ['true', 'false']
      responses:
        '200':
          description: List of jobs
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/Job'

  /jobs/batch:
    post:
      tags: [Jobs]
      summary: Perform batch operation on jobs
      operationId: batchJobs
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/BatchJobRequest'
      responses:
        '200':
          description: Batch operation completed
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/CountResponse'
        '409':
          description: Conflict - some jobs are active
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ErrorResponse'

  /jobs/{id}/cancel:
    post:
      tags: [Jobs]
      summary: Cancel a job
      operationId: cancelJob
      parameters:
        - $ref: '#/components/parameters/JobIdParam'
      responses:
        '200':
          description: Job cancelled
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/StatusResponse'
        '404':
          description: Job not found
        '409':
          description: Job cannot be cancelled

  /jobs/{id}/restart:
    post:
      tags: [Jobs]
      summary: Restart a completed or failed job
      operationId: restartJob
      parameters:
        - $ref: '#/components/parameters/JobIdParam'
      responses:
        '200':
          description: Job restarted
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/StatusResponse'
        '404':
          description: Job not found
        '409':
          description: Job is currently active

  /jobs/{id}/delete:
    post:
      tags: [Jobs]
      summary: Delete a job
      operationId: deleteJob
      parameters:
        - $ref: '#/components/parameters/JobIdParam'
      responses:
        '200':
          description: Job deleted
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/StatusResponse'
        '404':
          description: Job not found
        '409':
          description: Job is currently active

  /jobs/{id}/priority:
    post:
      tags: [Jobs]
      summary: Update job priority
      operationId: setJobPriority
      parameters:
        - $ref: '#/components/parameters/JobIdParam'
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              required: [priority]
              properties:
                priority:
                  type: integer
      responses:
        '200':
          description: Priority updated
          content:
            application/json:
              schema:
                type: object
                properties:
                  id:
                    type: integer
                  priority:
                    type: integer

  /jobs/{id}/details:
    get:
      tags: [Jobs]
      summary: Get detailed job information
      operationId: getJobDetails
      parameters:
        - $ref: '#/components/parameters/JobIdParam'
      responses:
        '200':
          description: Job details
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/JobDetails'
        '404':
          description: Job not found

  /jobs/restart-failed:
    post:
      tags: [Jobs]
      summary: Restart all failed/cancelled jobs
      operationId: restartFailedJobs
      responses:
        '200':
          description: Jobs queued for retry
          content:
            application/json:
              schema:
                type: object
                properties:
                  count:
                    type: integer
                  message:
                    type: string

  /jobs/clear-completed:
    post:
      tags: [Jobs]
      summary: Clear completed jobs from queue
      description: Preserves historical stats while removing completed jobs from the active queue.
      operationId: clearCompletedJobs
      responses:
        '200':
          description: Jobs cleared
          content:
            application/json:
              schema:
                type: object
                properties:
                  count:
                    type: integer
                  message:
                    type: string

  # ============================================================================
  # STATS ENDPOINTS
  # ============================================================================
  /stats:
    get:
      tags: [Stats]
      summary: Get current job statistics
      operationId: getStats
      responses:
        '200':
          description: Current statistics
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Stats'

  /stats/aggregated:
    get:
      tags: [Stats]
      summary: Get aggregated statistics
      operationId: getAggregatedStats
      responses:
        '200':
          description: Aggregated statistics
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/AggregatedStats'

  /stats/daily:
    get:
      tags: [Stats]
      summary: Get daily statistics for last 30 days
      operationId: getDailyStats
      responses:
        '200':
          description: Daily statistics
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/DailyStats'

  /stats/detailed:
    get:
      tags: [Stats]
      summary: Get detailed encode statistics
      operationId: getDetailedStats
      responses:
        '200':
          description: Detailed encode statistics (last 50 jobs)
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/DetailedStats'

  /stats/savings:
    get:
      tags: [Stats]
      summary: Get storage savings summary
      operationId: getSavingsStats
      responses:
        '200':
          description: Storage savings breakdown
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/SavingsStats'

  # ============================================================================
  # LOGS ENDPOINTS
  # ============================================================================
  /logs/history:
    get:
      tags: [Logs]
      summary: Get system logs with pagination
      operationId: getLogHistory
      parameters:
        - $ref: '#/components/parameters/PageParam'
        - $ref: '#/components/parameters/LimitParam'
      responses:
        '200':
          description: Log entries
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/LogEntry'

  /logs:
    delete:
      tags: [Logs]
      summary: Clear all system logs
      operationId: clearLogs
      responses:
        '200':
          description: Logs cleared

  # ============================================================================
  # EVENTS ENDPOINT (SSE)
  # ============================================================================
  /events:
    get:
      tags: [Events]
      summary: Subscribe to real-time events
      description: |
        Server-Sent Events stream for real-time updates.
        
        ## Event Types
        
        - **log**: System/job log entry
        - **progress**: Job encoding progress
        - **status**: Job status change
        - **decision**: Transcoder decision
        - **lagged**: Client fell behind event stream
      operationId: subscribeEvents
      responses:
        '200':
          description: SSE event stream
          content:
            text/event-stream:
              schema:
                type: string
                description: |
                  Events are sent in SSE format:
                  ```
                  event: log
                  data: {"level":"info","job_id":123,"message":"Starting encode"}
                  
                  event: progress
                  data: {"job_id":123,"percentage":45,"time":"00:15:30"}
                  ```

  # ============================================================================
  # ENGINE ENDPOINTS
  # ============================================================================
  /engine/status:
    get:
      tags: [Engine]
      summary: Get engine status
      operationId: getEngineStatus
      responses:
        '200':
          description: Engine status
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/EngineStatus'

  /engine/mode:
    get:
      tags: [Engine]
      summary: Get engine mode configuration
      operationId: getEngineMode
      responses:
        '200':
          description: Engine mode details
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/EngineModeResponse'
    post:
      tags: [Engine]
      summary: Set engine mode
      operationId: setEngineMode
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/EngineModeRequest'
      responses:
        '200':
          description: Mode updated
          content:
            application/json:
              schema:
                type: object
                properties:
                  status:
                    type: string
                  mode:
                    $ref: '#/components/schemas/EngineMode'
                  concurrent_limit:
                    type: integer
                  is_manual_override:
                    type: boolean

  /engine/pause:
    post:
      tags: [Engine]
      summary: Pause the encoding engine
      operationId: pauseEngine
      responses:
        '200':
          description: Engine paused
          content:
            application/json:
              schema:
                type: object
                properties:
                  status:
                    type: string
                    example: paused

  /engine/resume:
    post:
      tags: [Engine]
      summary: Resume the encoding engine
      operationId: resumeEngine
      responses:
        '200':
          description: Engine resumed
          content:
            application/json:
              schema:
                type: object
                properties:
                  status:
                    type: string
                    example: running

  /engine/drain:
    post:
      tags: [Engine]
      summary: Drain the job queue
      description: Stop accepting new jobs while finishing active ones.
      operationId: drainEngine
      responses:
        '200':
          description: Engine draining
          content:
            application/json:
              schema:
                type: object
                properties:
                  status:
                    type: string
                    example: draining

  /engine/stop-drain:
    post:
      tags: [Engine]
      summary: Stop draining and resume normal operation
      operationId: stopDrain
      responses:
        '200':
          description: Drain stopped
          content:
            application/json:
              schema:
                type: object
                properties:
                  status:
                    type: string
                    example: running

  # ============================================================================
  # SETTINGS ENDPOINTS
  # ============================================================================
  /settings/transcode:
    get:
      tags: [Settings]
      summary: Get transcoding settings
      operationId: getTranscodeSettings
      responses:
        '200':
          description: Transcoding settings
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/TranscodeSettings'
    post:
      tags: [Settings]
      summary: Update transcoding settings
      operationId: updateTranscodeSettings
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/TranscodeSettings'
      responses:
        '200':
          description: Settings updated
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/TranscodeSettings'
        '400':
          description: Invalid settings
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ErrorResponse'

  /settings/system:
    get:
      tags: [Settings]
      summary: Get system settings
      operationId: getSystemSettings
      responses:
        '200':
          description: System settings
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/SystemSettings'
    post:
      tags: [Settings]
      summary: Update system settings
      operationId: updateSystemSettings
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/SystemSettings'
      responses:
        '200':
          description: Settings updated
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/SystemSettings'

  /settings/hardware:
    get:
      tags: [Settings]
      summary: Get hardware acceleration settings
      operationId: getHardwareSettings
      responses:
        '200':
          description: Hardware settings
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/HardwareSettings'
    post:
      tags: [Settings]
      summary: Update hardware acceleration settings
      operationId: updateHardwareSettings
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/HardwareSettings'
      responses:
        '200':
          description: Settings updated
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/HardwareSettings'

  /settings/bundle:
    get:
      tags: [Settings]
      summary: Get entire configuration bundle
      operationId: getConfigBundle
      responses:
        '200':
          description: Full configuration
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ConfigBundle'
    put:
      tags: [Settings]
      summary: Update entire configuration bundle
      operationId: updateConfigBundle
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/ConfigBundle'
      responses:
        '200':
          description: Configuration updated
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ConfigBundle'

  /settings/config:
    get:
      tags: [Settings]
      summary: Get raw TOML configuration
      operationId: getRawConfig
      responses:
        '200':
          description: Raw configuration
          content:
            application/json:
              schema:
                type: object
                properties:
                  raw_toml:
                    type: string
                  normalized:
                    type: object
    put:
      tags: [Settings]
      summary: Update raw TOML configuration
      operationId: updateRawConfig
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              required: [raw_toml]
              properties:
                raw_toml:
                  type: string
      responses:
        '200':
          description: Configuration updated
          content:
            application/json:
              schema:
                type: object
                properties:
                  raw_toml:
                    type: string
                  normalized:
                    type: object

  /settings/files:
    get:
      tags: [Settings]
      summary: Get file settings
      operationId: getFileSettings
      responses:
        '200':
          description: File settings
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/FileSettings'
    post:
      tags: [Settings]
      summary: Update file settings
      operationId: updateFileSettings
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/FileSettings'
      responses:
        '200':
          description: Settings updated
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/FileSettings'

  /settings/preferences/{key}:
    get:
      tags: [Settings]
      summary: Get user preference
      operationId: getPreference
      parameters:
        - name: key
          in: path
          required: true
          schema:
            type: string
      responses:
        '200':
          description: Preference value
          content:
            application/json:
              schema:
                type: object
                properties:
                  key:
                    type: string
                  value:
                    type: string
        '404':
          description: Preference not found
    post:
      tags: [Settings]
      summary: Set user preference
      operationId: setPreference
      parameters:
        - name: key
          in: path
          required: true
          schema:
            type: string
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              required: [value]
              properties:
                value:
                  type: string
      responses:
        '200':
          description: Preference saved
          content:
            application/json:
              schema:
                type: object
                properties:
                  key:
                    type: string
                  value:
                    type: string

  /settings/watch-dirs:
    get:
      tags: [Settings]
      summary: List watch directories
      operationId: listWatchDirs
      responses:
        '200':
          description: Watch directories
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/WatchDir'
    post:
      tags: [Settings]
      summary: Add watch directory
      operationId: addWatchDir
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              required: [path]
              properties:
                path:
                  type: string
                is_recursive:
                  type: boolean
                  default: true
      responses:
        '201':
          description: Watch directory added
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/WatchDir'

  /settings/watch-dirs/{id}:
    delete:
      tags: [Settings]
      summary: Remove watch directory
      operationId: removeWatchDir
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: integer
      responses:
        '200':
          description: Watch directory removed
        '404':
          description: Watch directory not found

  /settings/notifications:
    get:
      tags: [Settings]
      summary: List notification targets
      operationId: listNotificationTargets
      responses:
        '200':
          description: Notification targets
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/NotificationTarget'
    post:
      tags: [Settings]
      summary: Add notification target
      operationId: addNotificationTarget
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/NotificationTargetInput'
      responses:
        '201':
          description: Notification target added
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/NotificationTarget'

  /settings/notifications/{id}:
    delete:
      tags: [Settings]
      summary: Remove notification target
      operationId: removeNotificationTarget
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: integer
      responses:
        '200':
          description: Notification target removed
        '404':
          description: Notification target not found

  /settings/notifications/test:
    post:
      tags: [Settings]
      summary: Send test notification
      operationId: testNotification
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/NotificationTargetInput'
      responses:
        '200':
          description: Test notification sent
        '400':
          description: Invalid notification configuration

  /settings/schedule:
    get:
      tags: [Settings]
      summary: List schedule windows
      operationId: listScheduleWindows
      responses:
        '200':
          description: Schedule windows
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/ScheduleWindow'
    post:
      tags: [Settings]
      summary: Add schedule window
      operationId: addScheduleWindow
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/ScheduleWindowInput'
      responses:
        '201':
          description: Schedule window added
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ScheduleWindow'

  /settings/schedule/{id}:
    delete:
      tags: [Settings]
      summary: Remove schedule window
      operationId: removeScheduleWindow
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: integer
      responses:
        '200':
          description: Schedule window removed
        '404':
          description: Schedule window not found

  # ============================================================================
  # SCAN ENDPOINTS
  # ============================================================================
  /scan:
    post:
      tags: [Scan]
      summary: Trigger library scan (legacy)
      operationId: triggerScanLegacy
      responses:
        '200':
          description: Scan completed
        '202':
          description: Scan started

  /scan/start:
    post:
      tags: [Scan]
      summary: Start library scan
      operationId: startScan
      responses:
        '202':
          description: Scan started
          content:
            application/json:
              schema:
                type: object
                properties:
                  status:
                    type: string
                    example: accepted

  /scan/status:
    get:
      tags: [Scan]
      summary: Get scan status
      operationId: getScanStatus
      responses:
        '200':
          description: Scan status
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ScanStatus'

  # ============================================================================
  # PROFILES ENDPOINTS
  # ============================================================================
  /profiles/presets:
    get:
      tags: [Profiles]
      summary: Get built-in profile presets
      operationId: getProfilePresets
      responses:
        '200':
          description: Built-in presets
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/Profile'

  /profiles:
    get:
      tags: [Profiles]
      summary: List all profiles
      operationId: listProfiles
      responses:
        '200':
          description: All profiles
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/Profile'
    post:
      tags: [Profiles]
      summary: Create profile
      operationId: createProfile
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/ProfileInput'
      responses:
        '201':
          description: Profile created
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Profile'

  /profiles/{id}:
    put:
      tags: [Profiles]
      summary: Update profile
      operationId: updateProfile
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: integer
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/ProfileInput'
      responses:
        '200':
          description: Profile updated
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Profile'
        '404':
          description: Profile not found
    delete:
      tags: [Profiles]
      summary: Delete profile
      operationId: deleteProfile
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: integer
      responses:
        '200':
          description: Profile deleted
        '404':
          description: Profile not found
        '409':
          description: Profile is in use

  /watch-dirs/{id}/profile:
    patch:
      tags: [Profiles]
      summary: Assign profile to watch directory
      operationId: assignWatchDirProfile
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: integer
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              properties:
                profile_id:
                  type: integer
                  nullable: true
                  description: Profile ID to assign, or null to unassign
      responses:
        '200':
          description: Profile assigned
        '404':
          description: Watch directory not found

  # ============================================================================
  # LIBRARY HEALTH ENDPOINTS
  # ============================================================================
  /library/health:
    get:
      tags: [Library]
      summary: Get library health summary
      operationId: getLibraryHealth
      responses:
        '200':
          description: Health summary
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/LibraryHealth'

  /library/health/issues:
    get:
      tags: [Library]
      summary: Get jobs with health issues
      operationId: getLibraryHealthIssues
      responses:
        '200':
          description: Jobs with issues
          content:
            application/json:
              schema:
                type: array
                items:
                  type: object
                  properties:
                    job:
                      $ref: '#/components/schemas/Job'
                    report:
                      type: object
                      properties:
                        issues:
                          type: array
                          items:
                            type: object
                            properties:
                              type:
                                type: string
                                enum: [corruption, stream_issue]
                              severity:
                                type: string
                                enum: [critical, warning]
                              description:
                                type: string

  /library/health/scan:
    post:
      tags: [Library]
      summary: Start library health scan
      operationId: startHealthScan
      responses:
        '202':
          description: Health scan started
          content:
            application/json:
              schema:
                type: object
                properties:
                  status:
                    type: string
                    example: accepted

  /library/health/scan/{id}:
    post:
      tags: [Library]
      summary: Rescan specific job for health issues
      operationId: rescanJobHealth
      parameters:
        - $ref: '#/components/parameters/JobIdParam'
      responses:
        '200':
          description: Health scan result
          content:
            application/json:
              schema:
                type: object
                properties:
                  job_id:
                    type: integer
                  issue_found:
                    type: boolean

  # ============================================================================
  # SYSTEM ENDPOINTS
  # ============================================================================
  /health:
    get:
      tags: [System]
      summary: Health check
      operationId: healthCheck
      security: []
      responses:
        '200':
          description: Service is healthy
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/HealthResponse'

  /ready:
    get:
      tags: [System]
      summary: Readiness probe
      description: Checks database connectivity
      operationId: readinessProbe
      security: []
      responses:
        '200':
          description: Service is ready
          content:
            application/json:
              schema:
                type: object
                properties:
                  ready:
                    type: boolean
                    example: true
        '503':
          description: Service not ready
          content:
            application/json:
              schema:
                type: object
                properties:
                  ready:
                    type: boolean
                    example: false
                  reason:
                    type: string

  /system/resources:
    get:
      tags: [System]
      summary: Get system resource usage
      description: Returns real-time CPU, memory, disk usage. Cached for 500ms.
      operationId: getSystemResources
      responses:
        '200':
          description: System resources
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/SystemResources'

  /system/hardware:
    get:
      tags: [System]
      summary: Detect available hardware
      description: Detects GPUs and hardware encoders. Public during setup.
      operationId: detectHardware
      security: []
      responses:
        '200':
          description: Hardware detection results
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/HardwareInfo'

  /system/version:
    get:
      tags: [System]
      summary: Get version information
      operationId: getVersion
      responses:
        '200':
          description: Version info
          content:
            application/json:
              schema:
                type: object
                properties:
                  version:
                    type: string
                    example: 1.2.3
                  commit:
                    type: string
                  build_date:
                    type: string

  /system/ffmpeg:
    get:
      tags: [System]
      summary: Get FFmpeg information
      operationId: getFfmpegInfo
      responses:
        '200':
          description: FFmpeg info
          content:
            application/json:
              schema:
                type: object
                properties:
                  version:
                    type: string
                  path:
                    type: string
                  encoders:
                    type: array
                    items:
                      type: string
                  decoders:
                    type: array
                    items:
                      type: string

  # ============================================================================
  # FILESYSTEM ENDPOINTS
  # ============================================================================
  /fs/browse:
    get:
      tags: [Filesystem]
      summary: Browse filesystem
      operationId: browseFilesystem
      parameters:
        - name: path
          in: query
          schema:
            type: string
            default: /
      responses:
        '200':
          description: Directory listing
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/DirectoryListing'

  /fs/recommendations:
    get:
      tags: [Filesystem]
      summary: Get recommended directories
      operationId: getDirectoryRecommendations
      security: []
      responses:
        '200':
          description: Recommended directories
          content:
            application/json:
              schema:
                type: object
                properties:
                  input_directories:
                    type: array
                    items:
                      type: object
                      properties:
                        name:
                          type: string
                        path:
                          type: string
                  output_directory:
                    type: string
                  config_directory:
                    type: string

  /fs/preview:
    post:
      tags: [Filesystem]
      summary: Preview selected folders
      operationId: previewFolders
      security: []
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              properties:
                input_paths:
                  type: array
                  items:
                    type: string
                output_path:
                  type: string
                config_path:
                  type: string
      responses:
        '200':
          description: Folder preview
          content:
            application/json:
              schema:
                type: object
                properties:
                  valid:
                    type: boolean
                  issues:
                    type: array
                    items:
                      type: string
                  space_available_gb:
                    type: number
                  estimated_library_size:
                    type: integer

  # ============================================================================
  # SETUP ENDPOINTS
  # ============================================================================
  /setup/status:
    get:
      tags: [Setup]
      summary: Check if setup is required
      operationId: getSetupStatus
      security: []
      responses:
        '200':
          description: Setup status
          content:
            application/json:
              schema:
                type: object
                properties:
                  setup_required:
                    type: boolean
                  enable_telemetry:
                    type: boolean
                  config_mutable:
                    type: boolean

  /setup/complete:
    post:
      tags: [Setup]
      summary: Complete initial setup
      operationId: completeSetup
      security: []
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/SetupRequest'
      responses:
        '200':
          description: Setup completed
          content:
            application/json:
              schema:
                type: object
                properties:
                  status:
                    type: string
                  message:
                    type: string
        '400':
          description: Validation errors
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ErrorResponse'

  # ============================================================================
  # UI PREFERENCES ENDPOINTS
  # ============================================================================
  /ui/preferences:
    get:
      tags: [Settings]
      summary: Get UI preferences
      operationId: getUiPreferences
      responses:
        '200':
          description: UI preferences
          content:
            application/json:
              schema:
                type: object
                additionalProperties: true
    post:
      tags: [Settings]
      summary: Update UI preferences
      operationId: updateUiPreferences
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              additionalProperties: true
      responses:
        '200':
          description: Preferences updated
          content:
            application/json:
              schema:
                type: object
                additionalProperties: true

# ==============================================================================
# COMPONENTS
# ==============================================================================
components:
  securitySchemes:
    bearerAuth:
      type: http
      scheme: bearer
    cookieAuth:
      type: apiKey
      in: cookie
      name: alchemist_session

  parameters:
    JobIdParam:
      name: id
      in: path
      required: true
      schema:
        type: integer
    LimitParam:
      name: limit
      in: query
      schema:
        type: integer
        minimum: 1
        maximum: 200
        default: 50
    PageParam:
      name: page
      in: query
      schema:
        type: integer
        minimum: 1
        default: 1

  schemas:
    # --------------------------------------------------------------------------
    # Common Responses
    # --------------------------------------------------------------------------
    StatusResponse:
      type: object
      properties:
        status:
          type: string
          example: ok

    ErrorResponse:
      type: object
      properties:
        error:
          type: string
        message:
          type: string
        details:
          type: object

    CountResponse:
      type: object
      properties:
        count:
          type: integer

    # --------------------------------------------------------------------------
    # Auth
    # --------------------------------------------------------------------------
    LoginRequest:
      type: object
      required: [username, password]
      properties:
        username:
          type: string
        password:
          type: string
          format: password

    # --------------------------------------------------------------------------
    # Jobs
    # --------------------------------------------------------------------------
    Job:
      type: object
      properties:
        id:
          type: integer
        input_path:
          type: string
        output_path:
          type: string
          nullable: true
        status:
          $ref: '#/components/schemas/JobStatus'
        priority:
          type: integer
          default: 0
        created_at:
          type: string
          format: date-time
        updated_at:
          type: string
          format: date-time
        archived:
          type: boolean
          default: false

    JobStatus:
      type: string
      enum:
        - Queued
        - Analyzing
        - Encoding
        - Remuxing
        - Resuming
        - Completed
        - Failed
        - Cancelled
        - Archived

    JobDetails:
      type: object
      properties:
        job:
          $ref: '#/components/schemas/Job'
        metadata:
          type: object
          nullable: true
          description: FFmpeg metadata if available
        encode_stats:
          type: object
          nullable: true
          description: Detailed encode statistics
        job_logs:
          type: array
          items:
            $ref: '#/components/schemas/LogEntry'
        job_failure_summary:
          type: string
          nullable: true

    BatchJobRequest:
      type: object
      required: [action, ids]
      properties:
        action:
          type: string
          enum: [cancel, delete, restart]
        ids:
          type: array
          items:
            type: integer

    # --------------------------------------------------------------------------
    # Stats
    # --------------------------------------------------------------------------
    Stats:
      type: object
      properties:
        total:
          type: integer
        completed:
          type: integer
        active:
          type: integer
        failed:
          type: integer
        concurrent_limit:
          type: integer

    AggregatedStats:
      type: object
      properties:
        total_input_bytes:
          type: integer
          format: int64
        total_output_bytes:
          type: integer
          format: int64
        total_savings_bytes:
          type: integer
          format: int64
        total_time_seconds:
          type: integer
        total_jobs:
          type: integer
        avg_vmaf:
          type: number
          nullable: true

    DailyStats:
      type: object
      properties:
        date:
          type: string
          format: date
        jobs_completed:
          type: integer
        total_input_bytes:
          type: integer
          format: int64
        total_output_bytes:
          type: integer
          format: int64
        total_time_seconds:
          type: integer

    DetailedStats:
      type: object
      properties:
        job_id:
          type: integer
        input_size:
          type: integer
          format: int64
        output_size:
          type: integer
          format: int64
        encode_time_seconds:
          type: integer
        vmaf_score:
          type: number
          nullable: true
        codec:
          type: string
        quality:
          type: string

    SavingsStats:
      type: object
      properties:
        total_saved_bytes:
          type: integer
          format: int64
        by_codec:
          type: object
          additionalProperties:
            type: object
            properties:
              count:
                type: integer
              saved_bytes:
                type: integer
                format: int64
        by_quality:
          type: object
          additionalProperties:
            type: object
            properties:
              count:
                type: integer
              saved_bytes:
                type: integer
                format: int64

    # --------------------------------------------------------------------------
    # Logs
    # --------------------------------------------------------------------------
    LogEntry:
      type: object
      properties:
        id:
          type: integer
        level:
          type: string
          enum: [info, warn, error]
        job_id:
          type: integer
          nullable: true
        message:
          type: string
        timestamp:
          type: string
          format: date-time

    # --------------------------------------------------------------------------
    # Engine
    # --------------------------------------------------------------------------
    EngineStatus:
      type: object
      properties:
        status:
          type: string
          enum: [running, paused, draining]
        manual_paused:
          type: boolean
        scheduler_paused:
          type: boolean
        draining:
          type: boolean
        mode:
          $ref: '#/components/schemas/EngineMode'
        concurrent_limit:
          type: integer
        is_manual_override:
          type: boolean

    EngineMode:
      type: string
      enum: [Background, Balanced, Throughput]

    EngineModeResponse:
      type: object
      properties:
        mode:
          $ref: '#/components/schemas/EngineMode'
        is_manual_override:
          type: boolean
        concurrent_limit:
          type: integer
        cpu_count:
          type: integer
        computed_limits:
          type: object
          properties:
            background:
              type: integer
            balanced:
              type: integer
            throughput:
              type: integer

    EngineModeRequest:
      type: object
      required: [mode]
      properties:
        mode:
          $ref: '#/components/schemas/EngineMode'
        concurrent_jobs_override:
          type: integer
          nullable: true
        threads_override:
          type: integer
          nullable: true

    # --------------------------------------------------------------------------
    # Settings
    # --------------------------------------------------------------------------
    TranscodeSettings:
      type: object
      properties:
        concurrent_jobs:
          type: integer
          minimum: 1
        size_reduction_threshold:
          type: number
          minimum: 0
          maximum: 1
        min_bpp_threshold:
          type: number
        min_file_size_mb:
          type: integer
        output_codec:
          type: string
          enum: [h265, vp9, av1]
        quality_profile:
          type: string
          enum: [high, medium, low]
        threads:
          type: integer
          minimum: 0
          maximum: 512
          description: 0 = auto
        allow_fallback:
          type: boolean
        hdr_mode:
          type: string
          enum: [tone-map, passthrough, strip]
        tonemap_algorithm:
          type: string
          enum: [linear, gamma, bt2390]
        tonemap_peak:
          type: number
          minimum: 50
          maximum: 1000
        tonemap_desat:
          type: number
        subtitle_mode:
          type: string
          enum: [none, passthrough, burn]
        stream_rules:
          type: object
          description: Stream filtering rules

    SystemSettings:
      type: object
      properties:
        monitoring_poll_interval:
          type: number
          minimum: 0.5
          maximum: 10
        enable_telemetry:
          type: boolean
        watch_enabled:
          type: boolean

    HardwareSettings:
      type: object
      properties:
        allow_cpu_fallback:
          type: boolean
        allow_cpu_encoding:
          type: boolean
        cpu_preset:
          type: string
          enum: [slow, medium, fast, faster]
        preferred_vendor:
          type: string
          enum: [nvidia, intel, amd]
          nullable: true
        device_path:
          type: string
          nullable: true

    FileSettings:
      type: object
      additionalProperties: true

    ConfigBundle:
      type: object
      description: Complete configuration including all settings sections
      additionalProperties: true

    # --------------------------------------------------------------------------
    # Watch Directories
    # --------------------------------------------------------------------------
    WatchDir:
      type: object
      properties:
        id:
          type: integer
        path:
          type: string
        is_recursive:
          type: boolean
        profile_id:
          type: integer
          nullable: true
        created_at:
          type: string
          format: date-time
        updated_at:
          type: string
          format: date-time

    # --------------------------------------------------------------------------
    # Notifications
    # --------------------------------------------------------------------------
    NotificationTarget:
      type: object
      properties:
        id:
          type: integer
        name:
          type: string
        target_type:
          type: string
          enum: [webhook, discord, gotify]
        endpoint_url:
          type: string
        auth_token:
          type: string
          nullable: true
        events:
          type: string
          description: JSON array of event types
        enabled:
          type: boolean
        created_at:
          type: string
          format: date-time

    NotificationTargetInput:
      type: object
      required: [name, target_type, endpoint_url]
      properties:
        name:
          type: string
        target_type:
          type: string
          enum: [webhook, discord, gotify]
        endpoint_url:
          type: string
          format: uri
        auth_token:
          type: string
        events:
          type: array
          items:
            type: string
        enabled:
          type: boolean
          default: true

    # --------------------------------------------------------------------------
    # Schedule
    # --------------------------------------------------------------------------
    ScheduleWindow:
      type: object
      properties:
        id:
          type: integer
        start_time:
          type: string
          example: '09:00'
        end_time:
          type: string
          example: '17:00'
        days_of_week:
          type: string
          description: JSON array of day numbers (0=Sunday, 6=Saturday)
        enabled:
          type: boolean
        created_at:
          type: string
          format: date-time
        updated_at:
          type: string
          format: date-time

    ScheduleWindowInput:
      type: object
      required: [start_time, end_time, days_of_week]
      properties:
        start_time:
          type: string
          example: '09:00'
        end_time:
          type: string
          example: '17:00'
        days_of_week:
          type: array
          items:
            type: integer
            minimum: 0
            maximum: 6
        enabled:
          type: boolean
          default: true

    # --------------------------------------------------------------------------
    # Profiles
    # --------------------------------------------------------------------------
    Profile:
      type: object
      properties:
        id:
          type: integer
        name:
          type: string
        preset:
          type: string
        codec:
          type: string
        quality_profile:
          type: string
        hdr_mode:
          type: string
        audio_mode:
          type: string
        crf_override:
          type: integer
          nullable: true
        notes:
          type: string
          nullable: true
        builtin:
          type: boolean

    ProfileInput:
      type: object
      required: [name]
      properties:
        name:
          type: string
        preset:
          type: string
        codec:
          type: string
        quality_profile:
          type: string
        hdr_mode:
          type: string
        audio_mode:
          type: string
        crf_override:
          type: integer
        notes:
          type: string

    # --------------------------------------------------------------------------
    # Library Health
    # --------------------------------------------------------------------------
    LibraryHealth:
      type: object
      properties:
        files_checked:
          type: integer
        issues_found:
          type: integer
        last_scan_time:
          type: string
          format: date-time
          nullable: true
        status:
          type: string
          enum: [healthy, issues_detected]

    # --------------------------------------------------------------------------
    # Scan
    # --------------------------------------------------------------------------
    ScanStatus:
      type: object
      properties:
        scanning:
          type: boolean
        files_scanned:
          type: integer
        files_queued:
          type: integer
        start_time:
          type: string
          format: date-time
          nullable: true
        elapsed_seconds:
          type: integer

    # --------------------------------------------------------------------------
    # System
    # --------------------------------------------------------------------------
    HealthResponse:
      type: object
      properties:
        status:
          type: string
          example: ok
        version:
          type: string
        uptime:
          type: string
          example: 48h 30m 15s
        uptime_seconds:
          type: integer

    SystemResources:
      type: object
      properties:
        cpu_usage_percent:
          type: number
        memory_used_bytes:
          type: integer
          format: int64
        memory_total_bytes:
          type: integer
          format: int64
        disk_used_bytes:
          type: integer
          format: int64
        disk_total_bytes:
          type: integer
          format: int64
        gpu_usage_percent:
          type: number
          nullable: true
        gpu_memory_used_bytes:
          type: integer
          format: int64
          nullable: true

    HardwareInfo:
      type: object
      properties:
        gpus:
          type: array
          items:
            type: object
            properties:
              name:
                type: string
              vendor:
                type: string
              device_path:
                type: string
        encoders:
          type: array
          items:
            type: string
        cpu_cores:
          type: integer

    # --------------------------------------------------------------------------
    # Filesystem
    # --------------------------------------------------------------------------
    DirectoryListing:
      type: object
      properties:
        path:
          type: string
        parent:
          type: string
          nullable: true
        directories:
          type: array
          items:
            type: object
            properties:
              name:
                type: string
              path:
                type: string
        files:
          type: array
          items:
            type: object
            properties:
              name:
                type: string
              path:
                type: string
              size:
                type: integer
                format: int64

    # --------------------------------------------------------------------------
    # Setup
    # --------------------------------------------------------------------------
    SetupRequest:
      type: object
      required: [username, password, input_directories, output_directory]
      properties:
        username:
          type: string
          minLength: 1
        password:
          type: string
          format: password
          minLength: 8
        input_directories:
          type: array
          items:
            type: string
          minItems: 1
        output_directory:
          type: string
        enable_telemetry:
          type: boolean
          default: false

security:
  - bearerAuth: []
  - cookieAuth: []
```
