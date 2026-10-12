# 固定18条件の観測結果

外側p50/worstはms。各source・条件3trial。API/CLIは別境界、RSSは異なるsampling方法なのでinterface間の直接順位にしない。

| 条件 | 旧結果 | main結果 | 外側p50 旧→main ms | main worst ms | sampled RSS 旧→main MiB | output q 旧→main |
|---|---|---|---:|---:|---|---|
| photo-none | accepted:3 | accepted:3 | 34.8 → 34.7 | 37.0 | 95.3–95.6 → 95.4–95.7 | 85 → 85 |
| photo-loose | accepted:3 | accepted:3 | 136.9 → 62.4 | 63.7 | 95.8–96.3 → 95.8–96.3 | 100 → 100 |
| photo-tight | accepted:3 | accepted:3 | 51.2 → 355.0 | 358.6 | 92.8–92.9 → 93.5–93.6 | 60 → 60 |
| photo-impossible | budget-unmet:3 | budget-unmet:3 | 33.1 → 449.6 | 454.8 | 88.7–89.1 → 89.2–89.3 | none → none |
| ui-none | accepted:3 | accepted:3 | 25.9 → 25.9 | 27.2 | 84.2–84.4 → 84.2–84.5 | 80 → 80 |
| ui-loose | accepted:3 | accepted:3 | 55.5 → 18.0 | 19.2 | 84.4–84.5 → 84.3–84.5 | 100 → 100 |
| ui-tight | accepted:3 | accepted:3 | 85.8 → 351.5 | 352.5 | 84.1–84.4 → 84.4–84.9 | 60 → 60 |
| ui-impossible | budget-unmet:3 | budget-unmet:3 | 84.9 → 751.8 | 755.4 | 83.4–84.1 → 83.9–84.5 | none → none |
| large-none | accepted:3 | accepted:3 | 791.4 → 795.1 | 799.5 | 276.1–276.6 → 276.5–276.5 | 85 → 85 |
| large-loose | accepted:3 | accepted:3 | 3138.1 → 1637.7 | 1638.2 | 276.3–276.5 → 276.3–276.6 | 100 → 100 |
| large-tight | accepted:3 | resource-timeout:3 | 1017.7 → 5559.4 | 5566.4 | 276.4–276.6 → 276.5–276.9 | 60 → none |
| large-impossible | budget-unmet:3 | resource-timeout:3 | 927.4 → 5556.8 | 5576.4 | 276.2–276.4 → 276.4–276.5 | none → none |
| cli-build | accepted:3 | accepted:3 | 1672.5 → 1705.8 | 1722.2 | 198.5–250.6 → 190.3–250.2 | 85,80, → 85,80, |
| cli-ui-tight | accepted:3 | accepted:3 | 124.9 → 397.9 | 416.9 | 54.4–54.6 → 54.6–55.0 | 60 → 60 |
| avif-none | accepted:3 | accepted:3 | 102.5 → 104.7 | 115.5 | 93.2–93.3 → 93.1–93.4 | 60 → 60 |
| avif-tight | accepted:3 | resource-timeout:3 | 477.7 → 5057.4 | 5094.7 | 94.1–94.4 → 96.1–96.5 | 61 → none |
| api-range-tight | accepted:3 | accepted:3 | 502.6 → 2792.7 | 2821.6 | 268.2–268.4 → 268.0–268.3 | 60 → 60 |
| api-known-max | known-feasible-false-rejection:3 | accepted:3 | 7.2 → 10.8 | 11.6 | 82.3–82.7 → 82.3–82.8 | none → 100 |
