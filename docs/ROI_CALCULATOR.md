# ROI Calculator Methodology

This document explains the formulas behind `docs/roi-calculator.html` and the static ROI example in the README.

## Purpose

The ROI model helps answer:
1. How much transfer data is reduced by smaller output files.
2. What that reduction means in monthly/yearly CDN cost.
3. When additional encode time is offset by bandwidth savings.

## Inputs

Required for bandwidth savings:
- `image_deliveries_per_month`: number of image responses served per month.
- `avg_size_mb_before`: average file size before optimization (MB).
- `reduction_percent`: expected size reduction percentage with lazy-image.
- `cdn_cost_per_gb`: transfer price in USD per GB.

Optional for break-even model:
- `unique_images_generated_per_month`: number of unique images encoded.
- `extra_encode_ms_per_image`: extra encode time compared with sharp.
- `compute_cost_per_vcpu_hour`: effective compute price.

## Formulas

`transfer_before_gb = image_deliveries_per_month * avg_size_mb_before / 1024`

`transfer_after_gb = transfer_before_gb * (1 - reduction_percent / 100)`

`saved_gb = transfer_before_gb - transfer_after_gb`

`monthly_bandwidth_savings_usd = saved_gb * cdn_cost_per_gb`

`yearly_bandwidth_savings_usd = monthly_bandwidth_savings_usd * 12`

Break-even support model:

`extra_encode_hours = unique_images_generated_per_month * extra_encode_ms_per_image / (1000 * 60 * 60)`

`extra_encode_cost_usd = extra_encode_hours * compute_cost_per_vcpu_hour`

`net_monthly_savings_usd = monthly_bandwidth_savings_usd - extra_encode_cost_usd`

Per-image break-even views (approximation):

`savings_per_view_per_image = (avg_size_mb_before / 1024) * (reduction_percent / 100) * cdn_cost_per_gb`

`encode_cost_per_image = extra_encode_ms_per_image / (1000 * 60 * 60) * compute_cost_per_vcpu_hour`

`break_even_views_per_image = encode_cost_per_image / savings_per_view_per_image`

## Pricing Notes (Realistic Usage)

Use your actual invoice rate. CDN pricing varies by region, volume, and contract:
- AWS CloudFront is generally usage-based (`$/GB`) with regional and tiered pricing.
- Cloudflare often uses plan-based pricing and optional overage/usage components.
- Enterprise contracts may differ significantly from public list prices.

The README example uses `$0.085/GB` as a practical reference point, not a universal constant.

## Worked Example (README)

Assumptions:
- `avg_size_mb_before = 1.0`
- `reduction_percent = 25`
- `cdn_cost_per_gb = 0.085`

For `10,000,000` monthly deliveries:
- `transfer_before_gb = 9,765.63 GB`
- `saved_gb = 2,441.41 GB`
- `monthly_bandwidth_savings_usd = 207.52`

## Caveats

- Savings depend on cache hit rate and delivery patterns.
- If images are encoded once and served many times, bandwidth savings dominate.
- For highly dynamic images encoded on every request, include compute costs in your decision.
