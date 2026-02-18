# Shipping Oracle - On-chain Validator

The on-chain component for the Shipping Oracle.

## Overview
The on-chain component is an Aiken validator that governs shipment tracking UTxOs. It ensures that a tracking UTxO can only be consumed into a shipment datum when the oracle provides a valid status and the oracle payment is enforced.

## Modules and Contracts
- `validators/tracking.ak`: Main validator logic for spending tracking UTxOs.
- `lib/types.ak`: Datum/redeemer definitions and status constants used by the validator.
- `aiken.toml`: Project metadata and default config values (tracking price, payment address).

## Parameters
- `tracking_price`: The amount of lovelaces required to create a Tracking UTxO.
- `payment_address`: The address that will receive the funds once the shipment is closed.

## License

Licensed under the Apache License, Version 2.0. See `LICENSE`.
