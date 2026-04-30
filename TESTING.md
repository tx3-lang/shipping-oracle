# Testing — Shipping Oracle

Runbook para reproducir todas las pruebas del proyecto, de unit tests a E2E
on-chain. Pensado para correrlo de arriba abajo desde una checkout limpia.

## Matriz

| Nivel | Qué prueba | Comando | Red / cuenta | Tiempo |
|---|---|---|---|---|
| 1 | Unit + integration Rust | `cargo test` | ninguna (Shippo stubbed con wiremock) | ~30 s |
| 2 | Smoke de la API en vivo | `cargo run` + `scripts/smoke.sh` | Shippo real | ~5 s |
| 3 | Unit Aiken (validator + mint) | `aiken check` | ninguna | ~2 s |
| 4 | Build on-chain (two-pass) | `aiken build` x2 | ninguna | ~5 s |
| 5 | E2E local con devnet | `trix devnet` + tx3 | Dolos local (sin testnet) | ~1 min |

---

## 1. Tests del backend (sin red)

```bash
cd backend
cargo test                             # todo
cargo test --test cbor_alignment       # alineación CBOR vs Aiken
cargo test --test signature_vectors    # vectores Ed25519 deterministas
cargo test --test integration          # HTTP end-to-end con wiremock
```

No requiere `.env` — Shippo está stubbed.

---

## 2. Smoke de la API en vivo

### 2.1 Configurar el `.env`

```bash
cd backend
cp .env.example .env
```

Completar las 5 variables obligatorias:

| Variable | De dónde sale |
|---|---|
| `SHIPPO_API_KEY` | dashboard de Shippo |
| `ORACLE_SK` | hex de 32 bytes, ej. derivado de `local_wallets/oracle_wallet.skey` |
| `ORACLE_PKH` | blake2b-224 de la vkey (28 bytes hex) |
| `ORACLE_ADDRESS` | contenido de `local_wallets/oracle_wallet.addr` |
| `TRP_URL` | endpoint TRP, ej. `http://localhost:8164` |

### 2.2 Levantar el server

```bash
cd backend
cargo run
# → "oracle listening addr=0.0.0.0:3000"
```

### 2.3 Correr el smoke

En otra terminal:

```bash
./scripts/smoke.sh
# Overrides opcionales:
#   BASE_URL=http://host:port ./scripts/smoke.sh
#   CARRIER=ups ./scripts/smoke.sh   (tracking numbers reales, no demo)
```

El script pega a `/health` y a `/v1/shipment` con tres tracking numbers de
demo de Shippo: `SHIPPO_PRE_TRANSIT`, `SHIPPO_TRANSIT`, `SHIPPO_DELIVERED`.

Requiere `jq`.

---

## 3. Tests unitarios on-chain

```bash
cd onchain
aiken check                   # todos
aiken check -m oracle         # solo el withdrawal validator
aiken check -m governance_nft # solo la mint policy
```

Cubre:

- `oracle.ak`: `withdraw_valid_signature`, `withdraw_invalid_signature`,
  `withdraw_tampered_data`, `withdraw_missing_governance_nft`.
- `governance_nft.ak`: `mint_valid`, `mint_missing_seed_input`,
  `mint_wrong_asset_name`, `mint_wrong_quantity`, `mint_extra_asset`.
- `cbor_alignment_tests.ak`: bytes pinneados contra `backend/tests/cbor_alignment.rs`.

Si rompo un vector en Rust, hay que actualizar el lado Aiken al mismo tiempo
— los hex están hardcodeados en ambos.

---

## 4. Build on-chain (two-pass)

Hay un orden obligatorio: `oracle.ak` referencia `config.gov_policy_id`, y
ese hash depende del bytecode compilado de `governance_nft.ak`. Por eso
hace falta compilar dos veces.

```bash
cd onchain

# Pass 1: compila con el placeholder actual
aiken build

# Capturar el policy id real
GOV_POLICY_ID=$(jq -r '.validators[] | select(.title|contains("governance_nft.mint")) | .hash' plutus.json)
echo "$GOV_POLICY_ID"

# Reemplazar gov_policy_id en aiken.toml [config.default]
# Mantener encoding = "base16"
# Editar manualmente o:
#   sed -i.bak "s/^gov_policy_id = .*/gov_policy_id = { bytes = \"$GOV_POLICY_ID\", encoding = \"base16\" }/" aiken.toml

# Pass 2: re-compila con el policy id real
aiken build
```

Recordatorios:

- En `aiken.toml`, los bytes deben ser `{ bytes = "...", encoding = "base16" }`.
  Si omito `encoding`, `aiken check` falla con "missing field encoding".
- Cada vez que cambio `seed_utxo_*`, el `gov_policy_id` cambia → repetir el
  two-pass.

---

## 5. E2E local con trix devnet (Dolos)

> Los flags exactos de `trix` varían entre versiones. Si un comando no
> matchea, correr `trix --help` o `trix <subcommand> --help`.

### 5.1 Levantar el devnet

```bash
cd tx3
trix devnet start
trix devnet info     # muestra wallets pre-fondeadas (alice/bob/charlie)
```

`devnet.toml` define UTxOs pre-fondeadas para `@alice`, `@bob`, `@charlie`
(100k ADA cada una).

### 5.2 Cargar la wallet del oracle

```bash
trix wallet import oracle --skey local_wallets/oracle_wallet.skey
```

`backend/.env` (`ORACLE_SK`) debe apuntar a la misma sk.

### 5.3 Configurar `seed_utxo_ref` y rebuildear

```bash
trix utxos --wallet oracle    # elegir uno; copiar tx_hash e index
# Editar onchain/aiken.toml:
#   seed_utxo_tx_hash = { bytes = "<tx_hash>", encoding = "base16" }
#   seed_utxo_index   = <index>
cd onchain && aiken build     # pass 1 → nuevo gov_policy_id
# Actualizar gov_policy_id en aiken.toml (ver sección 4)
aiken build                   # pass 2
```

### 5.4 Publicar scripts

```bash
cd tx3
trix tx run publish_scripts --profile devnet
```

Publica `governance_nft` y `oracle` como reference scripts. Anotar las refs
(`txhash#ix`) que devuelve.

### 5.5 Bootstrap governance (mint del NFT)

```bash
trix tx run bootstrap_governance --profile devnet
```

Consume `seed_utxo_ref`, mintea el NFT y lo bloquea en una UTxO con
`GovernanceDatum { oracle_vk }`. Anotar `governance_utxo_ref`.

### 5.6 Levantar el backend contra el devnet

En `backend/.env`:

```
TRP_URL=http://localhost:8164    # endpoint TRP del devnet
ORACLE_SK=<misma sk que la wallet del oracle>
```

```bash
cd backend && cargo run
./scripts/smoke.sh    # confirmar que la API responde
```

### 5.7 Consumer tx (consume_oracle_data)

```bash
RESPONSE=$(curl -fsS "http://localhost:3000/v1/shipment?carrier=shippo&tracking_number=SHIPPO_DELIVERED")
CARRIER_HASH=$(jq -r '.data.carrier_hash' <<<"$RESPONSE")
TN_HASH=$(jq -r '.data.tracking_number_hash' <<<"$RESPONSE")
STATUS_HEX=$(printf '%s' "$(jq -r '.data.status' <<<"$RESPONSE")" | xxd -p)
TIMESTAMP=$(jq -r '.data.timestamp' <<<"$RESPONSE")
SIG=$(jq -r '.signature' <<<"$RESPONSE")

cd tx3
trix tx run consume_oracle_data \
  --profile devnet \
  --arg p_carrier_hash="$CARRIER_HASH" \
  --arg p_tracking_number_hash="$TN_HASH" \
  --arg p_status="$STATUS_HEX" \
  --arg p_timestamp="$TIMESTAMP" \
  --arg p_signature="$SIG"
```

### 5.8 Verificar on-chain

```bash
trix utxos --wallet consumer    # debería aparecer la UTxO `attested` con OracleData inline
trix tx <txhash>                # detalles de la tx
```

Si la firma no valida, el script falla y la tx es rechazada por el nodo —
ese es el smoke test on-chain.

---

## Gotchas

1. **Olvidar el segundo `aiken build`** después de cambiar `gov_policy_id` o
   `seed_utxo_*` en `aiken.toml` → on-chain compila con placeholder, falla
   en runtime.
2. **`oracle_vk` desincronizado** entre `backend/.env` (`ORACLE_SK`) y el
   `GovernanceDatum` mintado en 5.5 → la firma valida cripto-correctamente
   pero el validator la rechaza porque la vk del datum no es la que firmó.
   Re-mintear governance o re-configurar `ORACLE_SK`.
3. **`status` como string vs bytes**: el campo es `Bytes` on-chain. La API
   devuelve string (`"DELIVERED"`), pero al consumer hay que pasarle el hex
   de los bytes UTF-8. Por eso `xxd -p` en 5.7.
4. **Vectores de firma desincronizados**: si toco `signature_vectors.rs`
   tengo que re-pegar `test_oracle_vk` y `test_oracle_sig` en
   `onchain/validators/oracle.ak` (líneas 50-54). Pasa lo mismo con
   `cbor_alignment_tests.ak` cuando cambian los hex de `OracleData`.
