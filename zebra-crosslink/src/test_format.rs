use static_assertions::*;
use std::{io::Write, mem::align_of, mem::size_of};
use zebra_chain::serialization::{ZcashDeserialize, ZcashSerialize};
use zerocopy::*;
use zerocopy_derive::*;

use super::ZcashCrosslinkParameters;

#[repr(C)]
#[derive(Immutable, KnownLayout, IntoBytes, FromBytes)]
pub(crate) struct TFHdr {
    pub magic: [u8; 8],
    pub instrs_o: u64,
    pub instrs_n: u32,
    pub instr_size: u32, // used as stride
}

#[repr(C)]
#[derive(Clone, Copy, Immutable, IntoBytes, FromBytes)]
pub(crate) struct TFSlice {
    pub o: u64,
    pub size: u64,
}

impl TFSlice {
    pub(crate) fn as_val(self) -> [u64; 2] {
        [self.o, self.size]
    }

    pub(crate) fn as_byte_slice_in(self, bytes: &[u8]) -> &[u8] {
        &bytes[self.o as usize..(self.o + self.size) as usize]
    }
}

impl From<&[u64; 2]> for TFSlice {
    fn from(val: &[u64; 2]) -> TFSlice {
        TFSlice {
            o: val[0],
            size: val[1],
        }
    }
}

type TFInstrKind = u32;

#[repr(C)]
#[derive(Clone, Copy, Immutable, IntoBytes, FromBytes)]
pub(crate) struct TFInstr {
    pub kind: TFInstrKind,
    pub flags: u32,
    pub data: TFSlice,
    pub val: [u64; 2],
}

static TF_INSTR_KIND_STRS: [&str; TFInstr::COUNT as usize] = {
    let mut strs = [""; TFInstr::COUNT as usize];
    strs[TFInstr::LOAD_POW as usize] = "LOAD_POW";
    strs[TFInstr::LOAD_POS as usize] = "LOAD_POS";
    strs[TFInstr::SET_PARAMS as usize] = "SET_PARAMS";

    const_assert!(TFInstr::COUNT == 3);
    strs
};

impl TFInstr {
    // NOTE: we want to deal with unknown values at the *application* layer, not the
    // (de)serialization layer.
    // TODO: there may be a crate that makes an enum feasible here
    pub const LOAD_POW: TFInstrKind = 0;
    pub const LOAD_POS: TFInstrKind = 1;
    pub const SET_PARAMS: TFInstrKind = 2;
    pub const COUNT: TFInstrKind = 3;

    pub fn str_from_kind(kind: TFInstrKind) -> &'static str {
        let kind = kind as usize;
        if kind < TF_INSTR_KIND_STRS.len() {
            TF_INSTR_KIND_STRS[kind]
        } else {
            "<unknown>"
        }
    }

    pub fn data_slice<'a>(&self, bytes: &'a [u8]) -> &'a [u8] {
        self.data.as_byte_slice_in(bytes)
    }
}

pub(crate) struct TF {
    pub instrs: Vec<TFInstr>,
    pub data: Vec<u8>,
}

impl TF {
    pub(crate) fn new(params: &ZcashCrosslinkParameters) -> TF {
        let mut tf = TF {
            instrs: Vec::new(),
            data: Vec::new(),
        };

        // ALT: push as data & determine available info by size if we add more
        const_assert!(size_of::<ZcashCrosslinkParameters>() == 16);
        // enforce only 2 param members
        let ZcashCrosslinkParameters {
            bc_confirmation_depth_sigma,
            finalization_gap_bound,
        } = *params;
        let val = [bc_confirmation_depth_sigma, finalization_gap_bound];

        // NOTE:
        // This empty data slice results in a 0-length data at the current data offset... We could
        // also set it to 0-offset to clearly indicate there is no data intended to be used.
        // (Because the offset is from the beginning of the file, nothing will refer to valid
        // data at offset 0, which is the magic of the header)
        // TODO (once handled): tf.push_instr_ex(TFInstr::SET_PARAMS, 0, &[], val);

        tf
    }

    pub(crate) fn push_serialize<Z: ZcashSerialize>(&mut self, z: &Z) -> TFSlice {
        let bgn = (size_of::<TFHdr>() + self.data.len()) as u64;
        z.zcash_serialize(&mut self.data);
        let end = (size_of::<TFHdr>() + self.data.len()) as u64;

        TFSlice {
            o: bgn,
            size: end - bgn,
        }
    }

    pub(crate) fn push_data(&mut self, bytes: &[u8]) -> TFSlice {
        let result = TFSlice {
            o: (size_of::<TFHdr>() + self.data.len()) as u64,
            size: bytes.len() as u64,
        };
        self.data.write_all(bytes);
        result
    }

    pub(crate) fn push_instr_ex(
        &mut self,
        kind: TFInstrKind,
        flags: u32,
        data: &[u8],
        val: [u64; 2],
    ) {
        let data = self.push_data(data);
        self.instrs.push(TFInstr {
            kind,
            flags,
            data,
            val,
        });
    }

    pub(crate) fn push_instr(&mut self, kind: TFInstrKind, data: &[u8]) {
        self.push_instr_ex(kind, 0, data, [0; 2])
    }

    pub(crate) fn push_instr_serialize_ex<Z: ZcashSerialize>(
        &mut self,
        kind: TFInstrKind,
        flags: u32,
        data: &Z,
        val: [u64; 2],
    ) {
        let data = self.push_serialize(data);
        self.instrs.push(TFInstr {
            kind,
            flags,
            data,
            val,
        });
    }

    pub(crate) fn push_instr_serialize<Z: ZcashSerialize>(&mut self, kind: TFInstrKind, data: &Z) {
        self.push_instr_serialize_ex(kind, 0, data, [0; 2])
    }

    fn is_a_power_of_2(v: usize) -> bool {
        v != 0 && ((v & (v - 1)) == 0)
    }

    fn align_up(v: usize, mut align: usize) -> usize {
        assert!(Self::is_a_power_of_2(align));
        align -= 1;
        (v + align) & !align
    }

    pub(crate) fn write_to_file(&self, path: &std::path::Path) {
        if let Ok(mut file) = std::fs::File::create(path) {
            let instrs_o_unaligned = size_of::<TFHdr>() + self.data.len();
            let instrs_o = Self::align_up(instrs_o_unaligned, align_of::<TFInstr>());
            let hdr = TFHdr {
                magic: "ZECCLTF0".as_bytes().try_into().unwrap(),
                instrs_o: instrs_o as u64,
                instrs_n: self.instrs.len() as u32,
                instr_size: size_of::<TFInstr>() as u32,
            };
            file.write_all(hdr.as_bytes())
                .expect("writing shouldn't fail");
            file.write_all(&self.data).expect("writing shouldn't fail");

            if instrs_o > instrs_o_unaligned {
                const ALIGN_0S: [u8; align_of::<TFInstr>()] = [0u8; align_of::<TFInstr>()];
                let align_size = instrs_o - instrs_o_unaligned;
                let align_bytes = &ALIGN_0S[..align_size];
                file.write_all(align_bytes);
            }
            file.write_all(self.instrs.as_bytes())
                .expect("writing shouldn't fail");
        }
    }

    // Simple version, all in one go... for large files we'll want to break this up; get hdr &
    // get/stream instrs, then read data as needed
    pub(crate) fn read_from_file(path: &std::path::Path) -> Result<(Vec<u8>, Self), String> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) => return Err(err.to_string()),
        };

        let tf_hdr = match TFHdr::ref_from_prefix(&bytes[0..]) {
            Ok((hdr, _)) => hdr,
            Err(err) => return Err(err.to_string()),
        };

        let read_instrs = <[TFInstr]>::ref_from_prefix_with_elems(
            &bytes[tf_hdr.instrs_o as usize..],
            tf_hdr.instrs_n as usize,
        );

        let instrs = match read_instrs {
            Ok((instrs, _)) => instrs,
            Err(err) => return Err(err.to_string()),
        };

        let data = &bytes[size_of::<TFHdr>()..tf_hdr.instrs_o as usize];

        // TODO: just use slices, don't copy to vectors
        let tf = TF {
            instrs: instrs.to_vec(),
            data: data.to_vec(),
        };

        Ok((bytes, tf))
    }
}

use crate::*;

pub(crate) async fn instr_reader(internal_handle: TFLServiceHandle, path: std::path::PathBuf) {
    use zebra_chain::serialization::{ZcashDeserialize, ZcashSerialize};
    let call = internal_handle.call.clone();
    println!("Starting test");

    let (bytes, tf) = match TF::read_from_file(&path) {
        Err(err) => panic!("Invalid test file: {:?}: {}", path, err), // TODO: specifics
        Ok((bytes, tf)) => (bytes, tf),
    };

    for instr_i in 0..tf.instrs.len() {
        let instr = &tf.instrs[instr_i];
        info!(
            "Loading instruction {} ({})",
            TFInstr::str_from_kind(instr.kind),
            instr.kind
        );

        const_assert!(TFInstr::COUNT == 3);
        match instr.kind {
            TFInstr::LOAD_POW => {
                let block: Arc<Block> = Arc::new(
                    Block::zcash_deserialize(instr.data_slice(&bytes))
                        .expect("Serialization be valid"),
                );
                // NOTE (perf): block.hash() immediately reserializes the block to
                // hash the canonical form...
                let height_hash = (
                    block
                        .coinbase_height()
                        .expect("Block should have a valid height"),
                    block.hash(),
                );

                (call.force_feed_pow)(block).await;
                info!(
                    "Successfully loaded block at height {:?}, hash {}",
                    height_hash.0, height_hash.1
                );
            }

            TFInstr::LOAD_POS => {
                todo!("LOAD_POS");
            }

            TFInstr::SET_PARAMS => {
                debug_assert!(instr_i == 0, "should only be set at the beginning");
                // ALT: derive FromBytes for params
                let params = ZcashCrosslinkParameters {
                    bc_confirmation_depth_sigma: instr.val[0],
                    finalization_gap_bound: instr.val[1],
                };
            }

            _ => warn!("Unrecognized instruction {}", instr.kind),
        }
    }

    println!("Test done, shutting down");
    // zebrad::application::APPLICATION.shutdown(abscissa_core::Shutdown::Graceful);
    TEST_SHUTDOWN_FN.lock().unwrap()();
}
