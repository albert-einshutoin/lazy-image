# 同一座標cropの目視所見

[全画像の並列資料](visual-comparison.html)で参照と出力を同一座標・同一表示幅で確認した。
これは担当者の目視であり、被験者を用いた主観評価ではない。

- `coffee` の JPEG 50% 条件では、[参照crop](coffee-jpeg-50pct-reference-crop.png)に対して[出力crop](coffee-jpeg-50pct-output-crop.png)のカップ縁と泡の境界が柔らかく、周囲の細い反射が弱い。同じ参照に対するSSIMはbudgetなし0.95020から0.90583、PSNRは32.25から29.61 dBへ低下した。
- `chelsea` の WebP 50% 条件では、[参照crop](chelsea-webp-50pct-reference-crop.png)に対して[出力crop](chelsea-webp-50pct-output-crop.png)の目の周囲の細い毛並みがまとまり、輪郭の局所的な差が見える。SSIMは0.97920から0.94061、PSNRは37.27から33.44 dBへ低下した。
- 既存metadata専用入力の10 B best-effortでは、[参照crop](metadata-budget-best-effort-reference-crop.png)に対して[出力crop](metadata-budget-best-effort-output-crop.png)の細い文字の境界がぼける。出力は3,078 Bで`budgetMet:false`。これは不達を含む有効画像の品質評価であり、10 B達成例ではない。

同じ入力・形式のbudgetなしも非可逆出力である。上記の所見と指標差は各画像を共通のlossless参照に照らして比較したもので、codec単独の劣化量や知覚的同等性を示さない。
