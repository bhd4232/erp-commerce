# Universal COD Statement — Schema Notes

See `docs/COURIER_CSV_SAMPLES.md` for sources, mapping rules, and usage.

## Columns

| # | Column | Type | Required | Notes |
| ---: | --- | --- | :---: | --- |
| 1 | `source` | text | yes | `pathao`, `steadfast`, `redx` |
| 2 | `statement_id` | text | yes | payout/statement id |
| 3 | `consignment_id` | text | yes | primary matching key |
| 4 | `merchant_invoice_id` | text | no | order/invoice id |
| 5 | `tracking_code` | text | no | customer-facing code |
| 6 | `recipient_name` | text | no | customer name |
| 7 | `recipient_phone` | text | no | normalized 11-digit phone |
| 8 | `delivery_area` | text | no | courier area/city |
| 9 | `status` | text | yes | delivered/returned/in_transit/cancelled |
| 10 | `cod_amount` | decimal | yes | expected COD |
| 11 | `cod_collected` | decimal | yes | collected COD |
| 12 | `delivery_charge` | decimal | yes | delivery fee |
| 13 | `cod_fee` | decimal | yes | COD fee |
| 14 | `adjustment` | decimal | yes | other deduction/credit |
| 15 | `net_amount` | decimal | yes | amount due to merchant |
| 16 | `return_flag` | boolean | yes | `true` for returned |
| 17 | `delivered_at` | date | no | blank if not delivered |
| 18 | `payout_date` | date | no | statement/payout date |
| 19 | `mismatch_type` | text | no | filled by reconciliation |
| 20 | `mismatch_amount` | decimal | no | filled by reconciliation |
| 21 | `notes` | text | no | evidence/comment |

## Rules

- Money is decimal, never float.
- `recipient_phone` is normalized to 11 digits before matching.
- `mismatch_type`/`mismatch_amount` are output fields; courier parsers leave
  them blank.
- A duplicate `(source, statement_id, consignment_id)` line is collapsed, not
  double-counted.
- `pending` and `cancelled` never create a recovery case.
- Ambiguous matches go to `manual_review`.
