# Metrics Contract (decode / ops / encode)

## 計測境界
- **decode_time (ms)**: `decode_internal()` 開始からデコード完了まで。入力バイトを画像に起こす処理のみを計測。
- **process_time (ms)**: デコード完了直後から `apply_ops` 完了まで。リサイズや回転などのオペレーションのみを計測。
- **encode_time (ms)**: エンコード開始からバイト列生成完了まで。形式変換や圧縮を含む。
- **processing_time (s)**: パイプライン開始から終了までのウォールクロック。`decode+process+encode` の合算を必ず包含する（同一 `Instant` 基準）。

## リソース系
- **cpu_time (s)**: `getrusage()` で取得した CPU 時間差分。取得できない環境では 0。
- **memory_peak (bytes)**: `ru_maxrss` を優先。取得不可の場合は `width*height*4 + output_size` で概算し、`u32::MAX` にクランプ。

## サイズ系
- **input_size (bytes)**: 入力ソースのバイト長（u32 クランプ）。
- **output_size (bytes)**: エンコード結果のバイト長（u32 クランプ）。
- **compression_ratio**: `output_size / input_size`。入力サイズが 0 の場合は 0。

## 保障事項
1) すべての時間計測は単一の `Instant` 基準で測定し、`processing_time` は decode/process/encode 区間を必ず包含する。  
2) 数値は非負。`decode_time + process_time + encode_time > 0` の場合、`processing_time*1000` はその合計以上になる。  
3) メタデータ（input/output/compression_ratio/cpu_time/memory_peak）は、計測可能な範囲で常に埋められる。取得不可項目は 0 にフォールバック。  
4) フォーマットやオプションにかかわらず測定の粒度・順序は一定。
