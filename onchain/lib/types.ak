use aiken/crypto.{VerificationKeyHash}
use cardano/address.{Address}

pub type TrackingDatum {
  carrier: ByteArray,
  tracking_number: ByteArray,
  outbox_address: Address,
}

pub type ShipmentDatum {
  carrier: ByteArray,
  tracking_number: ByteArray,
  status: ByteArray,
  timestamp: Int,
  oracle_pkh: VerificationKeyHash,
}

pub type TrackingRedeemer {
  ConsumeTracking
}

pub const status_delivered: ByteArray = "DELIVERED"

pub const status_not_delivered: ByteArray = "NOT_DELIVERED"
