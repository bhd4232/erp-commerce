# Courier CSV Samples — Universal COD Reconciliation Fixture

**Status:** reference/test fixture  
**Date:** 2026-11-22  
**Purpose:** give the COD platform a stable, synthetic set of courier order and COD
statement files to build parsers and reconciliation tests against.

> These are **synthetic samples**, not customer data. The courier merchant-panel
> CSV templates are behind a login; the files below are reconstructed from
> public API documentation and public integration guidance, then normalized into
> one universal statement schema. Always compare the header row with the
> merchant panel's own sample before production use.

---

## 1. Files

| File | What it is | Source basis |
| --- | --- | --- |
| `tests/fixtures/courier/pathao_bulk_orders.csv` | Pathao-style bulk order upload | Pathao public bulk-order API fields and public pathao plugin guidance |
| `tests/fixtures/courier/steadfast_bulk_orders.csv` | Steadfast-style bulk order upload | Steadfast public booking API fields |
| `tests/fixtures/courier/redx_bulk_orders.csv` | RedX-style bulk parcel upload | RedX public parcel API fields |
| `tests/fixtures/courier/universal_cod_statement.csv` | One normalized statement across all three couriers | Reconciliation model in `COD_PLATFORM_PLAN.md` |
| `tests/fixtures/courier/universal_cod_statement_schema.md` | Column definitions and mapping rules | This document |

---

## 2. Public sources used

- Pathao bulk orders: <https://github.com/refatbd/bangladesh-api-docs/blob/main/docs/pathao/05-bulk-orders.md>
- Pathao merchant plugin (bulk order creation): <https://github.com/pathao-eng/courier-woocommerce-plugin>
- Pathao bulk-order guidance (column order, city/zone): <https://banglacodes.com/blog/courier-order-file?lang=en>
- Steadfast booking API: <https://github.com/refatbd/bangladesh-api-docs/blob/main/docs/steadfast/02-booking-parcels.md>
- Steadfast payments/COD settlement: <https://github.com/refatbd/bangladesh-api-docs/blob/main/docs/steadfast/05-balance-and-payments.md>
- RedX parcel API: <https://github.com/refatbd/bangladesh-api-docs/blob/main/docs/redx/02-parcels-and-orders.md>
- RedX developer API: <https://redx.com.bd/developer-api>

Retrieved: **2026-11-22**. Public docs can change; re-check before each courier
connector release.

---

## 3. Universal statement schema

The universal file is the contract the reconciliation engine consumes. A
courier-specific parser must produce these fields; missing fields are left
blank, never invented.

| Column | Type | Meaning |
| --- | --- | --- |
| `source` | text | `pathao`, `steadfast`, or `redx` |
| `statement_id` | text | courier payout/statement id |
| `consignment_id` | text | courier parcel/consignment id |
| `merchant_invoice_id` | text | merchant order/invoice id |
| `tracking_code` | text | customer-facing tracking code |
| `recipient_name` | text | customer name |
| `recipient_phone` | text | normalized 11-digit phone |
| `delivery_area` | text | area/city label from the courier |
| `status` | text | delivered, returned, in_transit, cancelled |
| `cod_amount` | decimal | expected COD to collect |
| `cod_collected` | decimal | COD the courier says it collected |
| `delivery_charge` | decimal | courier delivery fee |
| `cod_fee` | decimal | courier COD fee |
| `adjustment` | decimal | any other deduction/credit |
| `net_amount` | decimal | amount due to merchant |
| `return_flag` | boolean | `true` for a returned parcel |
| `delivered_at` | date | delivery date, blank if not delivered |
| `payout_date` | date | courier payout/statement date |
| `mismatch_type` | text | filled by reconciliation, not the courier |
| `mismatch_amount` | decimal | filled by reconciliation |
| `notes` | text | evidence/comment |

### Matching order

1. `consignment_id` exact match.
2. `merchant_invoice_id` + `source`.
3. normalized `recipient_phone` + `cod_amount` + date window.
4. manual review; never auto-create a recovery case on an ambiguous match.

### Mismatch types

- `missing_cod` — delivered but no COD collected/remitted.
- `underpaid` — collected less than expected.
- `overpaid` — collected more than expected.
- `overcharged_fee` — delivery/COD fee higher than the agreed rate.
- `return_fee` — return charge not explained by the statement.
- `duplicate_line` — same consignment appears twice.
- `prepaid_fee` — fee on a prepaid order with zero COD.
- `pending` — not delivered yet; not a recovery case.
- `cancelled` — cancelled before pickup; no recovery.
- `manual_review` — ambiguous; human decides.

---

## 4. Courier-specific field mapping

### Pathao bulk order

Pathao reads the bulk file by **column position**; do not reorder columns. The
merchant panel's own sample is authoritative.

| Universal/order field | Pathao column |
| --- | --- |
| store id | `StoreId` |
| merchant order id | `MerchantOrderId` |
| recipient name | `RecipientName` |
| phone | `RecipientPhone` |
| address | `RecipientAddress` |
| city | `RecipientCity` |
| zone | `RecipientZone` |
| area | `RecipientArea` |
| delivery type | `DeliveryType` |
| item type | `ItemType` |
| instruction | `SpecialInstruction` |
| quantity | `ItemQuantity` |
| weight (kg) | `ItemWeight` |
| COD | `AmountToCollect` |
| description | `ItemDescription` |

### Steadfast bulk order

| Universal/order field | Steadfast column |
| --- | --- |
| merchant order id | `invoice` |
| recipient name | `recipient_name` |
| phone | `recipient_phone` |
| address | `recipient_address` |
| COD | `cod_amount` |
| note | `note` |

### RedX bulk parcel

| Universal/order field | RedX field |
| --- | --- |
| recipient name | `customer_name` |
| phone | `customer_phone` |
| area | `delivery_area` |
| area id | `delivery_area_id` |
| address | `customer_address` |
| merchant order id | `merchant_invoice_id` |
| COD | `cash_collection_amount` |
| weight (grams) | `parcel_weight` |
| declared value | `value` |
| instruction | `instruction` |
| closed box | `is_closed_box` |
| pickup store | `pickup_store_id` |

### Steadfast payout statement

The payout detail response provides `consignment_id`, `invoice`,
`cod_collected`, `delivery_charge`, `cod_fee`, and `net_amount`. Map `invoice`
to `merchant_invoice_id`; `payment_id` to `statement_id`.

### Pathao/RedX statements

Use the merchant panel export as the source. Map its consignment/tracking id to
`consignment_id`, invoice/merchant order to `merchant_invoice_id`, collected COD
to `cod_collected`, and fee columns to `delivery_charge`/`cod_fee`. Keep unknown
columns in `notes` until the parser is updated.

---

## 5. How to use these fixtures

- Unit test each parser against its courier file.
- Feed `universal_cod_statement.csv` into the reconciliation service.
- Assert:
  - exact matches produce no recovery case;
  - `missing_cod` and `underpaid` produce a case with the right amount;
  - duplicate lines are collapsed, not double-counted;
  - `pending`/`cancelled` never produce a recovery case;
  - tenant isolation holds when the same fixture is imported by two tenants.

Do not treat these files as production truth. They are a stable test contract
and a starting point for the universal schema.
