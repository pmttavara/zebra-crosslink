#![allow(unexpected_cfgs, unused, missing_docs)]
use std::sync::Arc;

use bytes::{Bytes, BytesMut};

use malachitebft_core_types::{
    Context, Extension, NilOrVal, Round, SignedExtension, ValidatorSet as _, VoteType, VotingPower,
};

use malachitebft_signing_ed25519::Signature;
use serde::{Deserialize, Serialize};

use malachitebft_proto::{Error as ProtoError, Protobuf};
use zebra_chain::serialization::ZcashDeserializeInto;

use core::fmt;

use malachitebft_core_types::{
    CertificateError, CommitCertificate, CommitSignature, PolkaSignature, SignedProposal,
    SignedProposalPart, SignedVote, SigningProvider, Validator as _,
};

pub use malachitebft_app::types::Keypair as MalKeyPair;
pub use malachitebft_core_consensus::{
    LocallyProposedValue as MalLocallyProposedValue, ProposedValue as MalProposedValue,
    VoteExtensionError as MalVoteExtensionError,
};
pub use malachitebft_core_types::{
    Round as MalRound, Validity as MalValidity, VoteExtensions as MalVoteExtensions,
};

pub use malachitebft_signing_ed25519::{
    Ed25519 as MalEd25519, PrivateKey as MalPrivateKey, PublicKey as MalPublicKey,
};

use prost::Message;

use malachitebft_app::engine::util::streaming::{StreamContent, StreamId, StreamMessage};
use malachitebft_codec::Codec;
use malachitebft_core_consensus::SignedConsensusMsg;
use malachitebft_core_types::{PolkaCertificate, VoteSet};
use malachitebft_sync::PeerId;

use super::{BftPayload, Blake3Hash};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MalStreamedProposalData {
    pub data_bytes: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Round")]
enum RoundDef {
    Nil,
    Some(u32),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MalStreamedProposalPart {
    Init(MalStreamedProposalInit),
    Data(MalStreamedProposalData),
    Fin(MalStreamedProposalFin),
}

impl MalStreamedProposalPart {
    pub fn get_type(&self) -> &'static str {
        match self {
            Self::Init(_) => "init",
            Self::Data(_) => "data",
            Self::Fin(_) => "fin",
        }
    }

    pub fn as_init(&self) -> Option<&MalStreamedProposalInit> {
        match self {
            Self::Init(init) => Some(init),
            _ => None,
        }
    }

    pub fn as_data(&self) -> Option<&MalStreamedProposalData> {
        match self {
            Self::Data(data) => Some(data),
            _ => None,
        }
    }

    pub fn as_fin(&self) -> Option<&MalStreamedProposalFin> {
        match self {
            Self::Fin(fin) => Some(fin),
            _ => None,
        }
    }

    pub fn to_sign_bytes(&self) -> Bytes {
        malachitebft_proto::Protobuf::to_bytes(self).unwrap()
    }
}

/// A part of a value for a height, round. Identified in this scope by the sequence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MalStreamedProposalInit {
    pub height: MalHeight,
    #[serde(with = "RoundDef")]
    pub round: Round,
    #[serde(with = "RoundDef")]
    pub pol_round: Round,
    pub proposer: MalAddress,
}

impl MalStreamedProposalInit {
    pub fn new(height: MalHeight, round: Round, pol_round: Round, proposer: MalAddress) -> Self {
        Self {
            height,
            round,
            pol_round,
            proposer,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MalStreamedProposalFin {
    pub signature: Signature,
}

impl MalStreamedProposalFin {
    pub fn new(signature: Signature) -> Self {
        Self { signature }
    }
}

impl malachitebft_core_types::ProposalPart<MalContext> for MalStreamedProposalPart {
    fn is_first(&self) -> bool {
        matches!(self, Self::Init(_))
    }

    fn is_last(&self) -> bool {
        matches!(self, Self::Fin(_))
    }
}

impl Protobuf for MalStreamedProposalPart {
    type Proto = malctx_schema_proto::StreamedProposalPart;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        use malctx_schema_proto::streamed_proposal_part::Part;

        let part = proto
            .part
            .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("part"))?;

        match part {
            Part::Init(init) => Ok(Self::Init(MalStreamedProposalInit {
                height: MalHeight::new(init.height),
                round: Round::new(init.round),
                pol_round: Round::from(init.pol_round),
                proposer: init
                    .proposer
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("proposer"))
                    .and_then(MalAddress::from_proto)?,
            })),
            Part::Data(data) => Ok(Self::Data(MalStreamedProposalData {
                data_bytes: data.data_bytes.to_vec(),
            })),
            Part::Fin(fin) => Ok(Self::Fin(MalStreamedProposalFin {
                signature: fin
                    .signature
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("signature"))
                    .and_then(mal_decode_signature)?,
            })),
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        use malctx_schema_proto::streamed_proposal_part::Part;

        match self {
            Self::Init(init) => Ok(Self::Proto {
                part: Some(Part::Init(malctx_schema_proto::StreamedProposalInit {
                    height: init.height.as_u64(),
                    round: init.round.as_u32().unwrap(),
                    pol_round: init.pol_round.as_u32(),
                    proposer: Some(init.proposer.to_proto()?),
                })),
            }),
            Self::Data(data) => Ok(Self::Proto {
                part: Some(Part::Data(malctx_schema_proto::StreamedProposalData {
                    data_bytes: data.data_bytes.clone().into(),
                })),
            }),
            Self::Fin(fin) => Ok(Self::Proto {
                part: Some(Part::Fin(malctx_schema_proto::StreamedProposalFin {
                    signature: Some(mal_encode_signature(&fin.signature)),
                })),
            }),
        }
    }
}

/// A blockchain height
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MalHeight(u64);

impl MalHeight {
    pub const fn new(height: u64) -> Self {
        Self(height)
    }

    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    pub fn increment(&self) -> Self {
        Self(self.0 + 1)
    }

    pub fn decrement(&self) -> Option<Self> {
        self.0.checked_sub(1).map(Self)
    }
}

impl Default for MalHeight {
    fn default() -> Self {
        malachitebft_core_types::Height::ZERO
    }
}

impl fmt::Display for MalHeight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Debug for MalHeight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MalHeight({})", self.0)
    }
}

impl malachitebft_core_types::Height for MalHeight {
    const ZERO: Self = Self(0);
    const INITIAL: Self = Self(1);

    fn increment_by(&self, n: u64) -> Self {
        Self(self.0 + n)
    }

    fn decrement_by(&self, n: u64) -> Option<Self> {
        Some(Self(self.0.saturating_sub(n)))
    }

    fn as_u64(&self) -> u64 {
        self.0
    }
}

impl Protobuf for MalHeight {
    type Proto = u64;

    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        Ok(Self(proto))
    }

    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(self.0)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MalAddress(
    #[serde(
        serialize_with = "hex::serde::serialize_upper",
        deserialize_with = "hex::serde::deserialize"
    )]
    [u8; Self::LENGTH],
);

impl MalAddress {
    const LENGTH: usize = 20;

    #[cfg_attr(coverage_nightly, coverage(off))]
    pub const fn new(value: [u8; Self::LENGTH]) -> Self {
        Self(value)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn from_public_key(public_key: &MalPublicKey) -> Self {
        let hash: [u8; 32] = {
            use sha3::{Digest, Keccak256};
            let mut hasher = Keccak256::new();
            hasher.update(public_key.as_bytes());
            hasher.finalize().into()
        };
        let mut address = [0; Self::LENGTH];
        address.copy_from_slice(&hash[..Self::LENGTH]);
        Self(address)
    }

    pub fn into_inner(self) -> [u8; Self::LENGTH] {
        self.0
    }
}

impl fmt::Display for MalAddress {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0.iter() {
            write!(f, "{:02X}", byte)?;
        }
        Ok(())
    }
}

impl fmt::Debug for MalAddress {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MalAddress({})", self)
    }
}

impl malachitebft_core_types::Address for MalAddress {}

impl Protobuf for MalAddress {
    type Proto = malctx_schema_proto::Address;

    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        if proto.value.len() != Self::LENGTH {
            return Err(ProtoError::Other(format!(
                "Invalid address length: expected {}, got {}",
                Self::LENGTH,
                proto.value.len()
            )));
        }

        let mut address = [0; Self::LENGTH];
        address.copy_from_slice(&proto.value);
        Ok(Self(address))
    }

    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(malctx_schema_proto::Address {
            value: self.0.to_vec().into(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Serialize, Deserialize)]
pub struct MalValueId(pub Blake3Hash);

impl std::fmt::Display for MalValueId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Protobuf for MalValueId {
    type Proto = malctx_schema_proto::ValueId;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        let bytes = proto
            .value
            .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("value"))?;

        let len = bytes.len();
        let bytes = <[u8; 32]>::try_from(bytes.as_ref()).map_err(|_| {
            ProtoError::Other(format!(
                "Invalid value id bytes length, got {len} bytes expected {}",
                32
            ))
        })?;

        Ok(MalValueId(Blake3Hash(bytes)))
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(malctx_schema_proto::ValueId {
            value: Some(self.0 .0.to_vec().into()),
        })
    }
}

/// The value to decide on
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MalValue {
    pub value: BftPayload,
}

impl MalValue {
    pub fn new(value: BftPayload) -> Self {
        Self { value }
    }

    pub fn id(&self) -> MalValueId {
        MalValueId(self.value.blake3_hash())
    }
}

impl malachitebft_core_types::Value for MalValue {
    type Id = MalValueId;

    fn id(&self) -> MalValueId {
        self.id()
    }
}

impl Protobuf for MalValue {
    type Proto = malctx_schema_proto::Value;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        use zebra_chain::serialization::ZcashDeserialize;
        let value_bytes = proto
            .value
            .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("value"))?;

        Ok(MalValue {
            value: value_bytes
                .zcash_deserialize_into()
                .map_err(|e| ProtoError::Other(format!("ZCashDeserializeError: {:?}", e)))?,
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        use bytes::BufMut;
        use zebra_chain::serialization::ZcashSerialize;

        Ok(malctx_schema_proto::Value {
            value: Some(self.value.zcash_serialize_to_vec().unwrap().into()),
        })
    }
}

#[derive(Debug)]
pub struct MalEd25519Provider {
    private_key: MalPrivateKey,
}

impl MalEd25519Provider {
    pub fn new(private_key: MalPrivateKey) -> Self {
        Self { private_key }
    }

    pub fn private_key(&self) -> &MalPrivateKey {
        &self.private_key
    }

    pub fn sign(&self, data: &[u8]) -> Signature {
        self.private_key.sign(data)
    }

    pub fn verify(&self, data: &[u8], signature: &Signature, public_key: &MalPublicKey) -> bool {
        public_key.verify(data, signature).is_ok()
    }
}

impl SigningProvider<MalContext> for MalEd25519Provider {
    fn sign_vote(&self, vote: MalVote) -> SignedVote<MalContext> {
        let signature = self.sign(&vote.to_sign_bytes());
        SignedVote::new(vote, signature)
    }

    fn verify_signed_vote(
        &self,
        vote: &MalVote,
        signature: &Signature,
        public_key: &MalPublicKey,
    ) -> bool {
        public_key.verify(&vote.to_sign_bytes(), signature).is_ok()
    }

    fn sign_proposal(&self, proposal: MalProposal) -> SignedProposal<MalContext> {
        let signature = self.private_key.sign(&proposal.to_sign_bytes());
        SignedProposal::new(proposal, signature)
    }

    fn verify_signed_proposal(
        &self,
        proposal: &MalProposal,
        signature: &Signature,
        public_key: &MalPublicKey,
    ) -> bool {
        public_key
            .verify(&proposal.to_sign_bytes(), signature)
            .is_ok()
    }

    fn sign_proposal_part(
        &self,
        proposal_part: MalStreamedProposalPart,
    ) -> SignedProposalPart<MalContext> {
        let signature = self.private_key.sign(&proposal_part.to_sign_bytes());
        SignedProposalPart::new(proposal_part, signature)
    }

    fn verify_signed_proposal_part(
        &self,
        proposal_part: &MalStreamedProposalPart,
        signature: &Signature,
        public_key: &MalPublicKey,
    ) -> bool {
        public_key
            .verify(&proposal_part.to_sign_bytes(), signature)
            .is_ok()
    }

    fn sign_vote_extension(&self, extension: Bytes) -> SignedExtension<MalContext> {
        let signature = self.private_key.sign(extension.as_ref());
        malachitebft_core_types::SignedMessage::new(extension, signature)
    }

    fn verify_signed_vote_extension(
        &self,
        extension: &Bytes,
        signature: &Signature,
        public_key: &MalPublicKey,
    ) -> bool {
        public_key.verify(extension.as_ref(), signature).is_ok()
    }
}

/// A validator is a public key and voting power
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MalValidator {
    pub address: MalAddress,
    pub public_key: MalPublicKey,
    pub voting_power: VotingPower,
}

impl MalValidator {
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn new(public_key: MalPublicKey, voting_power: VotingPower) -> Self {
        Self {
            address: MalAddress::from_public_key(&public_key),
            public_key,
            voting_power,
        }
    }
}

impl PartialOrd for MalValidator {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MalValidator {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.address.cmp(&other.address)
    }
}

impl malachitebft_core_types::Validator<MalContext> for MalValidator {
    fn address(&self) -> &MalAddress {
        &self.address
    }

    fn public_key(&self) -> &MalPublicKey {
        &self.public_key
    }

    fn voting_power(&self) -> VotingPower {
        self.voting_power
    }
}

/// A validator set contains a list of validators sorted by address.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MalValidatorSet {
    pub validators: Arc<Vec<MalValidator>>,
}

impl MalValidatorSet {
    pub fn new(validators: impl IntoIterator<Item = MalValidator>) -> Self {
        let mut validators: Vec<_> = validators.into_iter().collect();
        MalValidatorSet::sort_validators(&mut validators);

        assert!(!validators.is_empty());

        Self {
            validators: Arc::new(validators),
        }
    }

    /// The total voting power of the validator set
    pub fn total_voting_power(&self) -> VotingPower {
        self.validators.iter().map(|v| v.voting_power).sum()
    }

    /// Get a validator by its address
    pub fn get_by_address(&self, address: &MalAddress) -> Option<&MalValidator> {
        self.validators.iter().find(|v| &v.address == address)
    }

    pub fn get_by_public_key(&self, public_key: &MalPublicKey) -> Option<&MalValidator> {
        self.validators.iter().find(|v| &v.public_key == public_key)
    }

    /// In place sort and deduplication of a list of validators
    fn sort_validators(vals: &mut Vec<MalValidator>) {
        // Sort the validators according to the current Tendermint requirements
        //
        // use core::cmp::Reverse;
        //
        // (v. 0.34 -> first by validator power, descending, then by address, ascending)
        // vals.sort_unstable_by(|v1, v2| {
        //     let a = (Reverse(v1.voting_power), &v1.address);
        //     let b = (Reverse(v2.voting_power), &v2.address);
        //     a.cmp(&b)
        // });

        vals.dedup();
    }

    pub fn get_keys(&self) -> Vec<MalPublicKey> {
        self.validators.iter().map(|v| v.public_key).collect()
    }
}

impl malachitebft_core_types::ValidatorSet<MalContext> for MalValidatorSet {
    fn count(&self) -> usize {
        self.validators.len()
    }

    fn total_voting_power(&self) -> VotingPower {
        self.total_voting_power()
    }

    fn get_by_address(&self, address: &MalAddress) -> Option<&MalValidator> {
        self.get_by_address(address)
    }

    fn get_by_index(&self, index: usize) -> Option<&MalValidator> {
        self.validators.get(index)
    }
}

/// A proposal for a value in a round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MalProposal {
    pub height: MalHeight,
    pub round: Round,
    pub value: MalValue,
    pub pol_round: Round,
    pub validator_address: MalAddress,
}

impl MalProposal {
    pub fn new(
        height: MalHeight,
        round: Round,
        value: MalValue,
        pol_round: Round,
        validator_address: MalAddress,
    ) -> Self {
        Self {
            height,
            round,
            value,
            pol_round,
            validator_address,
        }
    }

    pub fn to_bytes(&self) -> Bytes {
        Protobuf::to_bytes(self).unwrap()
    }

    pub fn to_sign_bytes(&self) -> Bytes {
        Protobuf::to_bytes(self).unwrap()
    }
}

impl malachitebft_core_types::Proposal<MalContext> for MalProposal {
    fn height(&self) -> MalHeight {
        self.height
    }

    fn round(&self) -> Round {
        self.round
    }

    fn value(&self) -> &MalValue {
        &self.value
    }

    fn take_value(self) -> MalValue {
        self.value
    }

    fn pol_round(&self) -> Round {
        self.pol_round
    }

    fn validator_address(&self) -> &MalAddress {
        &self.validator_address
    }
}

impl Protobuf for MalProposal {
    type Proto = malctx_schema_proto::Proposal;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(Self::Proto {
            height: self.height.to_proto()?,
            round: self.round.as_u32().expect("round should not be nil"),
            value: Some(self.value.to_proto()?),
            pol_round: self.pol_round.as_u32(),
            validator_address: Some(self.validator_address.to_proto()?),
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        Ok(Self {
            height: MalHeight::from_proto(proto.height)?,
            round: Round::new(proto.round),
            value: MalValue::from_proto(
                proto
                    .value
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("value"))?,
            )?,
            pol_round: Round::from(proto.pol_round),
            validator_address: MalAddress::from_proto(
                proto
                    .validator_address
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("validator_address"))?,
            )?,
        })
    }
}

pub mod malctx_schema_proto {
    #![allow(missing_docs)]

    include!(concat!(env!("OUT_DIR"), "/mal_schema.rs"));
}

#[derive(Copy, Clone, Debug)]
pub struct MalProtobufCodec;

impl Codec<MalValue> for MalProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<MalValue, Self::Error> {
        Protobuf::from_bytes(&bytes)
    }

    fn encode(&self, msg: &MalValue) -> Result<Bytes, Self::Error> {
        Protobuf::to_bytes(msg)
    }
}

impl Codec<MalStreamedProposalPart> for MalProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<MalStreamedProposalPart, Self::Error> {
        Protobuf::from_bytes(&bytes)
    }

    fn encode(&self, msg: &MalStreamedProposalPart) -> Result<Bytes, Self::Error> {
        Protobuf::to_bytes(msg)
    }
}

impl Codec<Signature> for MalProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<Signature, Self::Error> {
        let proto = malctx_schema_proto::Signature::decode(bytes.as_ref())?;
        mal_decode_signature(proto)
    }

    fn encode(&self, msg: &Signature) -> Result<Bytes, Self::Error> {
        Ok(Bytes::from(
            malctx_schema_proto::Signature {
                bytes: Bytes::copy_from_slice(msg.to_bytes().as_ref()),
            }
            .encode_to_vec(),
        ))
    }
}

impl Codec<SignedConsensusMsg<MalContext>> for MalProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<SignedConsensusMsg<MalContext>, Self::Error> {
        let proto = malctx_schema_proto::SignedMessage::decode(bytes.as_ref())?;

        let signature = proto
            .signature
            .ok_or_else(|| {
                ProtoError::missing_field::<malctx_schema_proto::SignedMessage>("signature")
            })
            .and_then(mal_decode_signature)?;

        let proto_message = proto.message.ok_or_else(|| {
            ProtoError::missing_field::<malctx_schema_proto::SignedMessage>("message")
        })?;

        match proto_message {
            malctx_schema_proto::signed_message::Message::Proposal(proto) => {
                let proposal = MalProposal::from_proto(proto)?;
                Ok(SignedConsensusMsg::Proposal(SignedProposal::new(
                    proposal, signature,
                )))
            }
            malctx_schema_proto::signed_message::Message::Vote(vote) => {
                let vote = MalVote::from_proto(vote)?;
                Ok(SignedConsensusMsg::Vote(SignedVote::new(vote, signature)))
            }
        }
    }

    fn encode(&self, msg: &SignedConsensusMsg<MalContext>) -> Result<Bytes, Self::Error> {
        match msg {
            SignedConsensusMsg::Vote(vote) => {
                let proto = malctx_schema_proto::SignedMessage {
                    message: Some(malctx_schema_proto::signed_message::Message::Vote(
                        vote.message.to_proto()?,
                    )),
                    signature: Some(mal_encode_signature(&vote.signature)),
                };
                Ok(Bytes::from(proto.encode_to_vec()))
            }
            SignedConsensusMsg::Proposal(proposal) => {
                let proto = malctx_schema_proto::SignedMessage {
                    message: Some(malctx_schema_proto::signed_message::Message::Proposal(
                        proposal.message.to_proto()?,
                    )),
                    signature: Some(mal_encode_signature(&proposal.signature)),
                };
                Ok(Bytes::from(proto.encode_to_vec()))
            }
        }
    }
}

impl Codec<StreamMessage<MalStreamedProposalPart>> for MalProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<StreamMessage<MalStreamedProposalPart>, Self::Error> {
        let proto = malctx_schema_proto::StreamMessage::decode(bytes.as_ref())?;

        let proto_content = proto.content.ok_or_else(|| {
            ProtoError::missing_field::<malctx_schema_proto::StreamMessage>("content")
        })?;

        let content = match proto_content {
            malctx_schema_proto::stream_message::Content::Data(data) => {
                StreamContent::Data(MalStreamedProposalPart::from_bytes(&data)?)
            }
            malctx_schema_proto::stream_message::Content::Fin(_) => StreamContent::Fin,
        };

        Ok(StreamMessage {
            stream_id: StreamId::new(proto.stream_id),
            sequence: proto.sequence,
            content,
        })
    }

    fn encode(&self, msg: &StreamMessage<MalStreamedProposalPart>) -> Result<Bytes, Self::Error> {
        let proto = malctx_schema_proto::StreamMessage {
            stream_id: msg.stream_id.to_bytes(),
            sequence: msg.sequence,
            content: match &msg.content {
                StreamContent::Data(data) => Some(
                    malctx_schema_proto::stream_message::Content::Data(data.to_bytes()?),
                ),
                StreamContent::Fin => Some(malctx_schema_proto::stream_message::Content::Fin(true)),
            },
        };

        Ok(Bytes::from(proto.encode_to_vec()))
    }
}

impl Codec<MalProposedValue<MalContext>> for MalProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<MalProposedValue<MalContext>, Self::Error> {
        let proto = malctx_schema_proto::ProposedValue::decode(bytes.as_ref())?;

        let proposer = proto.proposer.ok_or_else(|| {
            ProtoError::missing_field::<malctx_schema_proto::ProposedValue>("proposer")
        })?;

        let value = proto.value.ok_or_else(|| {
            ProtoError::missing_field::<malctx_schema_proto::ProposedValue>("value")
        })?;

        Ok(MalProposedValue {
            height: MalHeight::new(proto.height),
            round: Round::new(proto.round),
            valid_round: proto.valid_round.map(Round::new).unwrap_or(Round::Nil),
            proposer: MalAddress::from_proto(proposer)?,
            value: MalValue::from_proto(value)?,
            validity: MalValidity::from_bool(proto.validity),
        })
    }

    fn encode(&self, msg: &MalProposedValue<MalContext>) -> Result<Bytes, Self::Error> {
        let proto = malctx_schema_proto::ProposedValue {
            height: msg.height.as_u64(),
            round: msg.round.as_u32().unwrap(),
            valid_round: msg.valid_round.as_u32(),
            proposer: Some(msg.proposer.to_proto()?),
            value: Some(msg.value.to_proto()?),
            validity: msg.validity.to_bool(),
        };

        Ok(Bytes::from(proto.encode_to_vec()))
    }
}

impl Codec<malachitebft_sync::Status<MalContext>> for MalProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<malachitebft_sync::Status<MalContext>, Self::Error> {
        let proto = malctx_schema_proto::Status::decode(bytes.as_ref())?;

        let proto_peer_id = proto
            .peer_id
            .ok_or_else(|| ProtoError::missing_field::<malctx_schema_proto::Status>("peer_id"))?;

        Ok(malachitebft_sync::Status {
            peer_id: PeerId::from_bytes(proto_peer_id.id.as_ref()).unwrap(),
            tip_height: MalHeight::new(proto.height),
            history_min_height: MalHeight::new(proto.earliest_height),
        })
    }

    fn encode(&self, msg: &malachitebft_sync::Status<MalContext>) -> Result<Bytes, Self::Error> {
        let proto = malctx_schema_proto::Status {
            peer_id: Some(malctx_schema_proto::PeerId {
                id: Bytes::from(msg.peer_id.to_bytes()),
            }),
            height: msg.tip_height.as_u64(),
            earliest_height: msg.history_min_height.as_u64(),
        };

        Ok(Bytes::from(proto.encode_to_vec()))
    }
}

impl Codec<malachitebft_sync::Request<MalContext>> for MalProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<malachitebft_sync::Request<MalContext>, Self::Error> {
        let proto = malctx_schema_proto::SyncRequest::decode(bytes.as_ref())?;
        let request = proto.request.ok_or_else(|| {
            ProtoError::missing_field::<malctx_schema_proto::SyncRequest>("request")
        })?;

        match request {
            malctx_schema_proto::sync_request::Request::ValueRequest(req) => {
                Ok(malachitebft_sync::Request::ValueRequest(
                    malachitebft_sync::ValueRequest::new(MalHeight::new(req.height)),
                ))
            }
            malctx_schema_proto::sync_request::Request::VoteSetRequest(req) => Ok(
                malachitebft_sync::Request::VoteSetRequest(malachitebft_sync::VoteSetRequest::new(
                    MalHeight::new(req.height),
                    Round::new(req.round),
                )),
            ),
        }
    }

    fn encode(&self, msg: &malachitebft_sync::Request<MalContext>) -> Result<Bytes, Self::Error> {
        let proto = match msg {
            malachitebft_sync::Request::ValueRequest(req) => malctx_schema_proto::SyncRequest {
                request: Some(malctx_schema_proto::sync_request::Request::ValueRequest(
                    malctx_schema_proto::ValueRequest {
                        height: req.height.as_u64(),
                    },
                )),
            },
            malachitebft_sync::Request::VoteSetRequest(req) => malctx_schema_proto::SyncRequest {
                request: Some(malctx_schema_proto::sync_request::Request::VoteSetRequest(
                    malctx_schema_proto::VoteSetRequest {
                        height: req.height.as_u64(),
                        round: req.round.as_u32().unwrap(),
                    },
                )),
            },
        };

        Ok(Bytes::from(proto.encode_to_vec()))
    }
}

impl Codec<malachitebft_sync::Response<MalContext>> for MalProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<malachitebft_sync::Response<MalContext>, Self::Error> {
        mal_decode_sync_response(malctx_schema_proto::SyncResponse::decode(bytes)?)
    }

    fn encode(
        &self,
        response: &malachitebft_sync::Response<MalContext>,
    ) -> Result<Bytes, Self::Error> {
        mal_encode_sync_response(response).map(|proto| proto.encode_to_vec().into())
    }
}

pub fn mal_decode_sync_response(
    proto_response: malctx_schema_proto::SyncResponse,
) -> Result<malachitebft_sync::Response<MalContext>, ProtoError> {
    let response = proto_response.response.ok_or_else(|| {
        ProtoError::missing_field::<malctx_schema_proto::SyncResponse>("messages")
    })?;

    let response = match response {
        malctx_schema_proto::sync_response::Response::ValueResponse(value_response) => {
            malachitebft_sync::Response::ValueResponse(malachitebft_sync::ValueResponse::new(
                MalHeight::new(value_response.height),
                value_response
                    .value
                    .map(mal_decode_synced_value)
                    .transpose()?,
            ))
        }
        malctx_schema_proto::sync_response::Response::VoteSetResponse(vote_set_response) => {
            let height = MalHeight::new(vote_set_response.height);
            let round = Round::new(vote_set_response.round);
            let vote_set = vote_set_response.vote_set.ok_or_else(|| {
                ProtoError::missing_field::<malctx_schema_proto::VoteSet>("vote_set")
            })?;

            malachitebft_sync::Response::VoteSetResponse(malachitebft_sync::VoteSetResponse::new(
                height,
                round,
                mal_decode_vote_set(vote_set)?,
                vote_set_response
                    .polka_certificates
                    .into_iter()
                    .map(decode_polka_certificate)
                    .collect::<Result<Vec<_>, _>>()?,
            ))
        }
    };
    Ok(response)
}

pub fn mal_encode_sync_response(
    response: &malachitebft_sync::Response<MalContext>,
) -> Result<malctx_schema_proto::SyncResponse, ProtoError> {
    let proto = match response {
        malachitebft_sync::Response::ValueResponse(value_response) => {
            malctx_schema_proto::SyncResponse {
                response: Some(malctx_schema_proto::sync_response::Response::ValueResponse(
                    malctx_schema_proto::ValueResponse {
                        height: value_response.height.as_u64(),
                        value: value_response
                            .value
                            .as_ref()
                            .map(mal_encode_synced_value)
                            .transpose()?,
                    },
                )),
            }
        }
        malachitebft_sync::Response::VoteSetResponse(vote_set_response) => {
            malctx_schema_proto::SyncResponse {
                response: Some(
                    malctx_schema_proto::sync_response::Response::VoteSetResponse(
                        malctx_schema_proto::VoteSetResponse {
                            height: vote_set_response.height.as_u64(),
                            round: vote_set_response
                                .round
                                .as_u32()
                                .expect("round should not be nil"),
                            vote_set: Some(mal_encode_vote_set(&vote_set_response.vote_set)?),
                            polka_certificates: vote_set_response
                                .polka_certificates
                                .iter()
                                .map(encode_polka_certificate)
                                .collect::<Result<Vec<_>, _>>()?,
                        },
                    ),
                ),
            }
        }
    };

    Ok(proto)
}

pub fn mal_encode_synced_value(
    synced_value: &malachitebft_sync::RawDecidedValue<MalContext>,
) -> Result<malctx_schema_proto::SyncedValue, ProtoError> {
    Ok(malctx_schema_proto::SyncedValue {
        value_bytes: synced_value.value_bytes.clone(),
        certificate: Some(mal_encode_commit_certificate(&synced_value.certificate)?),
    })
}

pub fn mal_decode_synced_value(
    proto: malctx_schema_proto::SyncedValue,
) -> Result<malachitebft_sync::RawDecidedValue<MalContext>, ProtoError> {
    let certificate = proto.certificate.ok_or_else(|| {
        ProtoError::missing_field::<malctx_schema_proto::SyncedValue>("certificate")
    })?;

    Ok(malachitebft_sync::RawDecidedValue {
        value_bytes: proto.value_bytes,
        certificate: mal_decode_commit_certificate(certificate)?,
    })
}

pub(crate) fn encode_polka_certificate(
    polka_certificate: &PolkaCertificate<MalContext>,
) -> Result<malctx_schema_proto::PolkaCertificate, ProtoError> {
    Ok(malctx_schema_proto::PolkaCertificate {
        height: polka_certificate.height.as_u64(),
        round: polka_certificate
            .round
            .as_u32()
            .expect("round should not be nil"),
        value_id: Some(polka_certificate.value_id.to_proto()?),
        signatures: polka_certificate
            .polka_signatures
            .iter()
            .map(
                |sig| -> Result<malctx_schema_proto::PolkaSignature, ProtoError> {
                    let address = sig.address.to_proto()?;
                    let signature = mal_encode_signature(&sig.signature);
                    Ok(malctx_schema_proto::PolkaSignature {
                        validator_address: Some(address),
                        signature: Some(signature),
                    })
                },
            )
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub(crate) fn decode_polka_certificate(
    certificate: malctx_schema_proto::PolkaCertificate,
) -> Result<PolkaCertificate<MalContext>, ProtoError> {
    let value_id = certificate
        .value_id
        .ok_or_else(|| {
            ProtoError::missing_field::<malctx_schema_proto::PolkaCertificate>("value_id")
        })
        .and_then(MalValueId::from_proto)?;

    Ok(PolkaCertificate {
        height: MalHeight::new(certificate.height),
        round: Round::new(certificate.round),
        value_id,
        polka_signatures: certificate
            .signatures
            .into_iter()
            .map(|sig| -> Result<PolkaSignature<MalContext>, ProtoError> {
                let address = sig.validator_address.ok_or_else(|| {
                    ProtoError::missing_field::<malctx_schema_proto::PolkaCertificate>(
                        "validator_address",
                    )
                })?;
                let signature = sig.signature.ok_or_else(|| {
                    ProtoError::missing_field::<malctx_schema_proto::PolkaCertificate>("signature")
                })?;
                let signature = mal_decode_signature(signature)?;
                let address = MalAddress::from_proto(address)?;
                Ok(PolkaSignature::new(address, signature))
            })
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub fn mal_decode_commit_certificate(
    certificate: malctx_schema_proto::CommitCertificate,
) -> Result<CommitCertificate<MalContext>, ProtoError> {
    let value_id = certificate
        .value_id
        .ok_or_else(|| {
            ProtoError::missing_field::<malctx_schema_proto::CommitCertificate>("value_id")
        })
        .and_then(MalValueId::from_proto)?;

    let commit_signatures = certificate
        .signatures
        .into_iter()
        .map(|sig| -> Result<CommitSignature<MalContext>, ProtoError> {
            let address = sig.validator_address.ok_or_else(|| {
                ProtoError::missing_field::<malctx_schema_proto::CommitCertificate>(
                    "validator_address",
                )
            })?;
            let signature = sig.signature.ok_or_else(|| {
                ProtoError::missing_field::<malctx_schema_proto::CommitCertificate>("signature")
            })?;
            let signature = mal_decode_signature(signature)?;
            let address = MalAddress::from_proto(address)?;
            Ok(CommitSignature::new(address, signature))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let certificate = CommitCertificate {
        height: MalHeight::new(certificate.height),
        round: Round::new(certificate.round),
        value_id,
        commit_signatures,
    };

    Ok(certificate)
}

pub fn mal_encode_commit_certificate(
    certificate: &CommitCertificate<MalContext>,
) -> Result<malctx_schema_proto::CommitCertificate, ProtoError> {
    Ok(malctx_schema_proto::CommitCertificate {
        height: certificate.height.as_u64(),
        round: certificate.round.as_u32().expect("round should not be nil"),
        value_id: Some(certificate.value_id.to_proto()?),
        signatures: certificate
            .commit_signatures
            .iter()
            .map(
                |sig| -> Result<malctx_schema_proto::CommitSignature, ProtoError> {
                    let address = sig.address.to_proto()?;
                    let signature = mal_encode_signature(&sig.signature);
                    Ok(malctx_schema_proto::CommitSignature {
                        validator_address: Some(address),
                        signature: Some(signature),
                    })
                },
            )
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub fn mal_decode_extension(
    ext: malctx_schema_proto::Extension,
) -> Result<SignedExtension<MalContext>, ProtoError> {
    let signature = ext
        .signature
        .ok_or_else(|| ProtoError::missing_field::<malctx_schema_proto::Extension>("signature"))
        .and_then(mal_decode_signature)?;

    Ok(SignedExtension::new(ext.data, signature))
}

pub fn mal_encode_extension(
    ext: &SignedExtension<MalContext>,
) -> Result<malctx_schema_proto::Extension, ProtoError> {
    Ok(malctx_schema_proto::Extension {
        data: ext.message.clone(),
        signature: Some(mal_encode_signature(&ext.signature)),
    })
}

pub fn mal_encode_vote_set(
    vote_set: &VoteSet<MalContext>,
) -> Result<malctx_schema_proto::VoteSet, ProtoError> {
    Ok(malctx_schema_proto::VoteSet {
        signed_votes: vote_set
            .votes
            .iter()
            .map(mal_encode_vote)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub fn mal_encode_vote(
    vote: &SignedVote<MalContext>,
) -> Result<malctx_schema_proto::SignedMessage, ProtoError> {
    Ok(malctx_schema_proto::SignedMessage {
        message: Some(malctx_schema_proto::signed_message::Message::Vote(
            vote.message.to_proto()?,
        )),
        signature: Some(mal_encode_signature(&vote.signature)),
    })
}

pub fn mal_decode_vote_set(
    vote_set: malctx_schema_proto::VoteSet,
) -> Result<VoteSet<MalContext>, ProtoError> {
    Ok(VoteSet {
        votes: vote_set
            .signed_votes
            .into_iter()
            .map(mal_decode_vote)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub fn mal_decode_vote(
    msg: malctx_schema_proto::SignedMessage,
) -> Result<SignedVote<MalContext>, ProtoError> {
    let signature = msg.signature.ok_or_else(|| {
        ProtoError::missing_field::<malctx_schema_proto::SignedMessage>("signature")
    })?;

    let vote = match msg.message {
        Some(malctx_schema_proto::signed_message::Message::Vote(v)) => Ok(v),
        _ => Err(ProtoError::Other(
            "Invalid message type: not a vote".to_string(),
        )),
    }?;

    let signature = mal_decode_signature(signature)?;
    let vote = MalVote::from_proto(vote)?;
    Ok(SignedVote::new(vote, signature))
}

pub fn mal_encode_signature(signature: &Signature) -> malctx_schema_proto::Signature {
    malctx_schema_proto::Signature {
        bytes: Bytes::copy_from_slice(signature.to_bytes().as_ref()),
    }
}

pub fn mal_decode_signature(
    signature: malctx_schema_proto::Signature,
) -> Result<Signature, ProtoError> {
    let bytes = <[u8; 64]>::try_from(signature.bytes.as_ref())
        .map_err(|_| ProtoError::Other("Invalid signature length".to_string()))?;
    Ok(Signature::from_bytes(bytes))
}

/// A vote for a value in a round
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MalVote {
    pub typ: VoteType,
    pub height: MalHeight,
    pub round: Round,
    pub value: NilOrVal<MalValueId>,
    pub validator_address: MalAddress,
    pub extension: Option<SignedExtension<MalContext>>,
}

impl MalVote {
    pub fn new_prevote(
        height: MalHeight,
        round: Round,
        value: NilOrVal<MalValueId>,
        validator_address: MalAddress,
    ) -> Self {
        Self {
            typ: VoteType::Prevote,
            height,
            round,
            value,
            validator_address,
            extension: None,
        }
    }

    pub fn new_precommit(
        height: MalHeight,
        round: Round,
        value: NilOrVal<MalValueId>,
        address: MalAddress,
    ) -> Self {
        Self {
            typ: VoteType::Precommit,
            height,
            round,
            value,
            validator_address: address,
            extension: None,
        }
    }

    pub fn to_bytes(&self) -> Bytes {
        Protobuf::to_bytes(self).unwrap()
    }

    pub fn to_sign_bytes(&self) -> Bytes {
        let vote = Self {
            extension: None,
            ..self.clone()
        };

        Protobuf::to_bytes(&vote).unwrap()
    }
}

impl Protobuf for MalVote {
    type Proto = malctx_schema_proto::Vote;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, ProtoError> {
        Ok(Self {
            typ: mal_decode_votetype(proto.vote_type()),
            height: MalHeight::from_proto(proto.height)?,
            round: Round::new(proto.round),
            value: match proto.value {
                Some(value) => NilOrVal::Val(MalValueId::from_proto(value)?),
                None => NilOrVal::Nil,
            },
            validator_address: MalAddress::from_proto(
                proto
                    .validator_address
                    .ok_or_else(|| ProtoError::missing_field::<Self::Proto>("validator_address"))?,
            )?,
            extension: Default::default(),
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, ProtoError> {
        Ok(Self::Proto {
            vote_type: mal_encode_votetype(self.typ).into(),
            height: self.height.to_proto()?,
            round: self.round.as_u32().expect("round should not be nil"),
            value: match &self.value {
                NilOrVal::Nil => None,
                NilOrVal::Val(v) => Some(v.to_proto()?),
            },
            validator_address: Some(self.validator_address.to_proto()?),
        })
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn mal_encode_votetype(vote_type: VoteType) -> malctx_schema_proto::VoteType {
    match vote_type {
        VoteType::Prevote => malctx_schema_proto::VoteType::Prevote,
        VoteType::Precommit => malctx_schema_proto::VoteType::Precommit,
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn mal_decode_votetype(vote_type: malctx_schema_proto::VoteType) -> VoteType {
    match vote_type {
        malctx_schema_proto::VoteType::Prevote => VoteType::Prevote,
        malctx_schema_proto::VoteType::Precommit => VoteType::Precommit,
    }
}

#[derive(Clone, Debug)]
pub struct MalContext;

impl malachitebft_core_types::Vote<MalContext> for MalVote {
    fn height(&self) -> MalHeight {
        self.height
    }

    fn round(&self) -> Round {
        self.round
    }

    fn value(&self) -> &NilOrVal<MalValueId> {
        &self.value
    }

    fn take_value(self) -> NilOrVal<MalValueId> {
        self.value
    }

    fn vote_type(&self) -> VoteType {
        self.typ
    }

    fn validator_address(&self) -> &MalAddress {
        &self.validator_address
    }

    fn extension(&self) -> Option<&SignedExtension<MalContext>> {
        self.extension.as_ref()
    }

    fn take_extension(&mut self) -> Option<SignedExtension<MalContext>> {
        self.extension.take()
    }

    fn extend(self, extension: SignedExtension<MalContext>) -> Self {
        Self {
            extension: Some(extension),
            ..self
        }
    }
}

impl Default for MalContext {
    fn default() -> Self {
        Self
    }
}

impl Context for MalContext {
    type Address = MalAddress;
    type ProposalPart = MalStreamedProposalPart;
    type Height = MalHeight;
    type Proposal = MalProposal;
    type ValidatorSet = MalValidatorSet;
    type Validator = MalValidator;
    type Value = MalValue;
    type Vote = MalVote;
    type Extension = Bytes;
    type SigningScheme = MalEd25519;

    fn select_proposer<'a>(
        &self,
        validator_set: &'a Self::ValidatorSet,
        height: Self::Height,
        round: Round,
    ) -> &'a Self::Validator {
        assert!(validator_set.count() > 0);
        assert!(round != Round::Nil && round.as_i64() >= 0);

        let proposer_index = {
            let height = height.as_u64() as usize;
            let round = round.as_i64() as usize;

            (height - 1 + round) % validator_set.count()
        };

        validator_set
            .get_by_index(proposer_index)
            .expect("proposer_index is valid")
    }

    fn new_proposal(
        &self,
        height: MalHeight,
        round: Round,
        value: MalValue,
        pol_round: Round,
        address: MalAddress,
    ) -> MalProposal {
        MalProposal::new(height, round, value, pol_round, address)
    }

    fn new_prevote(
        &self,
        height: MalHeight,
        round: Round,
        value_id: NilOrVal<MalValueId>,
        address: MalAddress,
    ) -> MalVote {
        MalVote::new_prevote(height, round, value_id, address)
    }

    fn new_precommit(
        &self,
        height: MalHeight,
        round: Round,
        value_id: NilOrVal<MalValueId>,
        address: MalAddress,
    ) -> MalVote {
        MalVote::new_precommit(height, round, value_id, address)
    }
}
