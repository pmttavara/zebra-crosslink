use std::{
    io::Write,
    mem::size_of,
};
use zebra_chain::{
    serialization::{
        ZcashDeserialize,
        ZcashSerialize,
    },
};
use zerocopy::*;
use zerocopy_derive::*;


#[repr(C)]
#[derive(Immutable, IntoBytes)]
pub(crate) struct TFHdr {
    pub magic: [u8; 8],
    pub instrs_o: u64,
    pub instrs_n: u32,
    pub instr_size: u32, // used as stride
}

#[repr(C)]
#[derive(Clone, Copy, Immutable, IntoBytes)]
pub(crate) struct TFSlice {
    pub o: u64,
    pub size: u64,
}

impl TFSlice {
    pub(crate) fn as_val(self) -> [u64; 2] {
        [self.o, self.size]
    }
}

impl From<&[u64; 2]> for TFSlice {
    fn from(val: &[u64; 2]) -> TFSlice {
        TFSlice { o: val[0], size: val[1] }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Immutable, IntoBytes)]
pub(crate) enum TFInstrKind {
    LoadPoW,
    LoadPoS,
}

#[repr(C)]
#[derive(Clone, Copy, Immutable, IntoBytes)]
pub(crate) struct TFInstr {
    pub kind: TFInstrKind,
    pub flags: u32,
    pub data: TFSlice,
    pub val: [u64; 2],
}

pub(crate) struct TF {
    pub instrs: Vec<TFInstr>,
    pub data: Vec<u8>,
}

impl TF {
    pub(crate) fn new() -> TF {
        TF {
            instrs: Vec::new(),
            data: Vec::new(),
        }
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

    pub(crate) fn push_instr_ex(&mut self, kind: TFInstrKind, flags: u32, data: &[u8], val: [u64; 2]) {
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

    pub(crate) fn push_instr_serialize_ex<Z: ZcashSerialize>(&mut self, kind: TFInstrKind, flags: u32, data: &Z, val: [u64; 2]) {
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
                instrs_o: (size_of::<TFHdr>() + self.data.len())  as u64,
                instrs_n: self.instrs.len() as u32,
                instr_size: size_of::<TFInstr>() as u32,
            };
            file.write_all(hdr.as_bytes()).expect("writing shouldn't fail");
            file.write_all(&self.data);
            file.write_all(self.instrs.as_bytes()).expect("writing shouldn't fail");
        }
    }
}
