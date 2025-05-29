use static_assertions::*;
use std::{io::Write, mem::size_of};
use zebra_chain::serialization::{ZcashDeserialize, ZcashSerialize};
use zerocopy::*;
use zerocopy_derive::*;

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
    pub(crate) fn new(params: &zebra_crosslink_chain::ZcashCrosslinkParameters) -> TF {
        let mut tf = TF {
            instrs: Vec::new(),
            data: Vec::new(),
        };

        // ALT: push as data & determine available info by size if we add more
        const_assert!(size_of::<zebra_crosslink_chain::ZcashCrosslinkParameters>() == 16);
        // enforce only 2 param members
        let zebra_crosslink_chain::ZcashCrosslinkParameters {
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

    pub(crate) fn write_to_file(&self, path: &str) {
        if let Ok(mut file) = std::fs::File::create(path) {
            let hdr = TFHdr {
                magic: "ZECCLTF0".as_bytes().try_into().unwrap(),
                instrs_o: (size_of::<TFHdr>() + self.data.len()) as u64,
                instrs_n: self.instrs.len() as u32,
                instr_size: size_of::<TFInstr>() as u32,
            };
            file.write_all(hdr.as_bytes())
                .expect("writing shouldn't fail");
            file.write_all(&self.data);
            file.write_all(self.instrs.as_bytes())
                .expect("writing shouldn't fail");
        }
    }

    // Simple version, all in one go... for large files we'll want to break this up; get hdr &
    // get/stream instrs, then read data as needed
    pub(crate) fn read_from_file(path: &str) -> (Option<Vec<u8>>, Option<Self>) {
        let bytes = if let Ok(bytes) = std::fs::read(path) {
            bytes
        } else {
            return (None, None);
        };

        let tf_hdr = if let Ok((hdr, _)) = TFHdr::ref_from_prefix(&bytes[0..]) {
            hdr
        } else {
            return (Some(bytes), None);
        };

        let instrs = if let Ok((instrs, _)) = <[TFInstr]>::ref_from_prefix_with_elems(
            &bytes[tf_hdr.instrs_o as usize..],
            tf_hdr.instrs_n as usize,
        ) {
            instrs
        } else {
            return (Some(bytes), None);
        };

        let data = &bytes[size_of::<TFHdr>()..tf_hdr.instrs_o as usize];

        // TODO: just use slices, don't copy to vectors
        let tf = TF {
            instrs: instrs.to_vec(),
            data: data.to_vec(),
        };

        (Some(bytes), Some(tf))
    }
}
