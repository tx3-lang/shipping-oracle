pub type GovernanceDatum {
  oracle_vk: ByteArray,
}

pub type OracleData {
  carrier_hash: ByteArray,
  tracking_number_hash: ByteArray,
  status: ByteArray,
  timestamp: Int,
}

pub type OracleRedeemer {
  data: OracleData,
  signature: ByteArray,
}

pub type EscrowDatum {
  buyer: ByteArray,
  merchant: ByteArray,
  order_id: ByteArray,
  carrier_hash: ByteArray,
  tracking_number_hash: ByteArray,
  paid_at: Int,
  refund_after: Int,
}

pub type EscrowRedeemer {
  Release { data: OracleData, signature: ByteArray }
  Refund
}

pub const status_delivered: ByteArray = "DELIVERED"

pub const status_not_delivered: ByteArray = "NOT_DELIVERED"

pub const status_in_transit: ByteArray = "IN_TRANSIT"

pub const status_pre_transit: ByteArray = "PRE_TRANSIT"

pub const status_unknown: ByteArray = "UNKNOWN"
