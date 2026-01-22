# Zero-Copy 定義と検証方法

このドキュメントは lazy-image が主張する「ゼロコピー」について、**意味・適用範囲・測定方法**を明確にします。

## 定義（意味）

- **ゼロコピーとは**: `fromPath()` または `processBatch()` で入力を受け取り、`toFile()`/`toBuffer()` で出力するまでに **Node.js の JS ヒープへ入力ファイル全体をコピーしない** ことを指す。  
- 具体的には、入力ファイルは **mmap（メモリマップ）** で Rust 側から直接参照され、JS 側にはファイルの生データを載せない。

## 適用範囲（どこで有効か）

有効:
- `ImageEngine.fromPath(...)` → 任意の処理 → `toFile()/toBuffer()/toBufferWithMetrics()`
- `processBatch()` の内部（各入力を mmap で扱う）
- Rust 側でのデコード・エンコード処理（ピクセルバッファは Rust メモリ内で管理）

無効/例外:
- `ImageEngine.from(Buffer)` / `fromBytes()` / `fromMemory()` など、JS でバッファを保持したまま渡す経路（受け取ったバッファを共有するため JS ヒープ依存）。
- `as_vec()` のように明示的に `Vec<u8>` 化する経路（ドキュメント内コメントにも「zero-copy を破る」と記載）。
- 出力バッファ（`toBuffer*` の戻り値）は Node.js `Buffer` として確保されるため、**出力分のコピーは発生**する。
- Windows では mmap 中のファイル削除ができない制約がある（動作はゼロコピーだがファイル運用に注意）。

## 測定可能な基準（数値目標）

1. **JS ヒープ増加**: `fromPath → toBufferWithMetrics` のパイプラインで **heapUsed の増加が 2MB 以下**（GC 可能状態で測定）。  
2. **RSS 予算式**: ピーク RSS は以下を満たすことを目標とする。  
   `peak_rss ≤ decoded_bytes + 24MB`  
   - `decoded_bytes = width × height × bpp` (`bpp`: JPEG=3, PNG/WebP/AVIF=4)  
   - 24MB はデコード/エンコード補助バッファとスレッド分の安全マージン。
3. **例**: 6000×4000 PNG (24MP, bpp=4) の場合  
   - `decoded_bytes ≈ 96MB` → 目標 `peak_rss ≤ 120MB`

この数値は **測定手順に従って再現・検証可能** であり、ズレがあれば issue/PR で調整する。

## 測定手順

1. Node を GC 可能モードで起動: `node --expose-gc docs/scripts/measure-zero-copy.js`
2. 出力例（JSON）:
   ```json
   {
     "source": "test_4.5MB_5000x5000.png",
     "rss_start_mb": 30.1,
     "rss_end_mb": 118.4,
     "rss_delta_mb": 88.3,
     "heap_delta_mb": 0.7,
     "peak_rss_metrics_mb": 116.9
   }
   ```
3. 判定:
   - `heap_delta_mb <= 2.0` を満たすこと
   - `rss_end_mb` が「予算式 + 10% バッファ」以内であること（例では 120MB ×1.1 ≒ 132MB → OK）

## FAQ

- **なぜ JS ヒープを指標にするのか?**  
  ゼロコピーの主張は「入力を JS ヒープに載せない」ことにあるため、ヒープ増加が事実上の証拠となる。
- **出力バッファはコピーになるのでは?**  
  はい。エンコード結果は必ず `Buffer` として生成されるため、出力サイズ分のメモリは必要。ゼロコピーの対象は「入力経路」である。
- **ストリーミング API は?**  
  デフォルトはディスクバッファを使うが、入力ストリームを JS で保持する場合はゼロコピーの対象外。ただし内部処理は同じメモリモデルを使う。

## まとめ

- **ゼロコピー = 入力ファイルを JS ヒープへコピーしない**（mmap で Rust から直接読む）
- **測定式**で上限を示し、`docs/scripts/measure-zero-copy.js` でいつでも再検証できる
- 適用範囲と例外を明示し、期待値と境界をドキュメント化

## mmap 中にファイルが変更・削除された場合の挙動

- **契約**: 処理中に元ファイルを変更・削除しないことを前提とする（変更は未定義動作）。
- **起こり得る結果**: デコード失敗、破損画像、OS 依存の SIGBUS/SIGSEGV（Linux/macOS）、Windows では削除自体が失敗。
- **推奨策**:
  - 変更が懸念される環境では `from(Buffer)` などコピー経路を使うか、事前に一時ディレクトリへコピーしてから処理する。
  - 共有ストレージでの並行書き込みを防ぐ場合は OS ロック（`flock` 相当）を使用する。
  - Windows では mmap 中に削除できないため、処理完了までファイルを保持するか、`from(Buffer)` を使用する。

### Windows で安全に扱うパターン例

- **すぐ削除したい**: 
  ```js
  const buf = fs.readFileSync(src); // JSヒープ経路
  const out = await ImageEngine.from(buf).toFile(dst, 'jpeg', 80);
  fs.unlinkSync(src); // OK
  ```
- **テンポラリにコピーして処理**:
  ```js
  const tmp = path.join(os.tmpdir(), path.basename(src));
  fs.copyFileSync(src, tmp);
  await ImageEngine.fromPath(tmp).toFile(dst, 'jpeg', 80);
  fs.unlinkSync(tmp); // 元ファイルはそのまま
  ```
- **バッチ処理**: `processBatch()` 実行中は入力を残し、完了後に削除する（スコープが抜けて mmap が閉じたことを確認してから削除）。
