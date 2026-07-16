//! In-loop filter parameter search (deblock, CDEF, loop restoration, CCSO).
//!
//! Reference: `avm/av2/encoder/{picklpf,pickcdef,pickrst,pickccso}.c`. Filters are
//! signalled off initially so output stays decodable while stages come online.
