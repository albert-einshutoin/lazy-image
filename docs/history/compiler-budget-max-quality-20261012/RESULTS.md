# 明示maxQualityの固定条件結果

各条件3trial。API/CLIの外側時間とRSS samplingは別境界。CLI E123のcauseは未観測。

| 条件 | 結果 | 外側p50 / worst ms | sampled RSS MiB | 選択q / bytes |
|---|---|---:|---|---|
| large-compiler-default | resource-timeout:3 | 5550.2 / 5557.0 | 276.7–276.8 | none |
| large-compiler-cap | accepted:3 | 3347.9 / 4025.9 | 276.5–276.8 | 60 / 93782 |
| avif-compiler-default | resource-timeout:3 | 5061.1 / 5080.0 | 96.4–96.5 | none |
| avif-compiler-cap | accepted:3 | 97.2 / 97.9 | 93.4–93.8 | 60 / 12850 |
| large-cli-default | resource-cause-unobserved:3 | 5552.3 / 5584.1 | 205.6–207.4 | none |
| large-cli-cap | accepted:3 | 3204.4 / 3222.2 | 193.6–194.0 | 60 / 93782 |
| avif-cli-default | resource-cause-unobserved:3 | 5081.6 / 5118.0 | 66.5–67.1 | none |
| avif-cli-cap | accepted:3 | 129.2 / 133.8 | 62.6–62.7 | 60 / 12850 |
