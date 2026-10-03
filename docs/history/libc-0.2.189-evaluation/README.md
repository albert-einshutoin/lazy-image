# libc 0.2.189 compatibility evaluation (#790)

開始mainは `4ea709c4c14f98b50ee883a11ae2c5ba927b19cb`。着手時に後続commitなしを確認した。
本体・Fuzzのlockfileを0.2.189に揃え、追加差分はplatform層の回帰チェックと公開metricsの有限性チェックに限定した。
この変更はversion 1.4.2の**未公開候補**であり、新しいtag・npm publish・公開npm smokeは行わない。
採否・最終HEAD・候補hash・Hosted検証・merge後mainは [PR #790](https://github.com/albert-einshutoin/lazy-image/pull/790) の最終検証コメントに記録する。

## 解決版と範囲

| 依存 | 本体 | Fuzz |
|---|---|---|
| libc | 0.2.189 | 0.2.189 |
| oxipng | 10.1.1 | 10.1.1 |
| fast_image_resize | 6.1.0、default-features=false / rayon | 同じpath manifest、同じfeature指定 |
| bytemuck | 1.25.2 | 1.25.1 |
| napi / derive / backend | 3.12.2 / 3.6.3 / 6.1.2 | default-features=false、NAPIなし |

本体・Fuzzともmainとの差分はlibcのversion/checksumだけ。manifest、npm lock、CLI・sharp・js-yaml、公開packageのversionは変更しない。
広いcargo updateは行わず、Fuzzは `cargo +1.88.0 update --manifest-path fuzz/Cargo.toml -p libc --precise 0.2.189` で更新した。
`local-evidence.json` にlockfile SHA-256、実環境、テストsource SHA-256、結果を保存した。
`root-tree.txt` / `fuzz-tree.txt` は各manifestの `cargo +1.88.0 tree --locked -i libc` の結果。
build後も各treeとlockfileの版・hashを照合した。

## 上流変更と利用経路

[0.2.187〜0.2.189のchangelog](https://github.com/rust-lang/libc/blob/0.2.189/CHANGELOG.md) を利用経路に対応付けた。

| 上流変更・経路 | lazy-imageでの影響範囲と確認 |
|---|---|
| 不透明C型の表現修正（0.2.187） | 依存codecのFILE等のFFI宣言、rustixのDIR pointerに関係し得る。mozjpegの製品経路はnew_mem/new_readerであり、fdopenベースのpath APIを直接呼ばない。依存のbuild・既存codec/APIテストで互換性を確認する。旧版のUB/crashは再現していない |
| DIRのSend/Sync復帰（0.2.188） | 製品に直接DIRの利用なし。tempfile→rustixのlibc backendにはDir wrapperがあり、既存のwrapper所有権とtrait実装を確認した。今回unsafe implや型変換を追加しない。上流は将来のtrait削除を予告しており、復帰を一般的なthread安全保証とは扱わない |
| time_tの不安定設定変更 | `.cargo`、build.rs、workflowに旧RUST_LIBC_UNSTABLE_*・libc_unstable cfgなし。配布6 targetはいずれも64-bit。今回cfgを追加しない |
| Windows timeシンボル修正 | 配布対象はx64 MSVC。platformのresource usageはNone。Windows候補のロード・API/CLIを確認するが、time系全symbolの直接実行を主張しない |
| flock / getrusage / rusage | platform.rsの直接利用。flockはio/source.rsの大きなファイルの共有lock、getrusageはtasks/processing.rsのmetrics開始・終了snapshotへ接続。今回このproduction経路を変更しない |
| mmap・ファイルI/O | memmap2→libcのmmap/munmap（GNUではmmap64）、tempfile→rustix/errno/getrandom。ロック処理をplatform層で直接確認。小画像fromPath成功をmmap実行証拠とは扱わない |
| codec / allocator / binding | mozjpeg(-sys)、libavif-sys/rav1e、webpのbuild経路、tikv-jemallocator/sys、parking_lot_core、NAPIのdladdr/dlopen等。解決版を維持し、release build・既存API/Worker・候補smokeで確認する |
| その他の追加定義・修正 | SPARC/riscv32/NuttX/Android/Solarish等、未配布OS・archや未使用定義の変更を製品の不具合修正として数えない。全リリース項目がlazy-imageの動作改善を意味しない |

## OS境界と公開API

Unixロックの既存成功チェックに、別プロセスとの排他競合→非ブロッキング失敗→明示解放後の共有lock成功を追加。
親が一時ファイルを排他lockし、子が製品のtry_shared_lockを呼ぶ。stdout/stdinで段階を同期し、各待機に10秒timeout、失敗時にもkill/waitで後始末する。
256 MiB閾値、大きな画像fixture、lock失敗時のメモリ読込fallbackには手を加えない。

Linux/macOS等の対応cfgでresource usageがSomeであることを必須化し、CPU有限・非負、RSS正を確認する。
Linuxはru_maxrssのKiBを1024倍してbytes、macOSはbytesのまま。CPUはuser/systemの秒とmicrosecondsを合算。
公開metricsはCPU差分を非負へ制限、peakRssはbytesをu32上限へ制限する既存仕様を照合した。
Linuxの単位は [Linux man-pages](https://man7.org/linux/man-pages/man2/getrusage.2.html)、macOSは実機の `man 2 getrusage` と [XNU calcru](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_resource.c) を参照した。
Windows等のNoneを許容する既存仕様は維持。RSSの新旧一致・操作後の復帰は要求しない。

ローカル実行はmacOS 27.0 (26A428) arm64、Rust 1.88.0、SDK MacOSX26.5.sdk (26.5)、Apple clang 21.0.0、Node 24.2.0。
旧mainのlockfile（libc 0.2.186）に同じテスト補強だけを適用したbaselineと、0.2.189候補を比較した。
両者ともplatformテスト5 PASS / 0 FAIL（子helperは通常runでは1 ignored、親テストから明示実行）、canonical release build成功。
basic、from-path-async、edge-cases、artifact-compiler、metadata-options、error-codesの6ファイルは両者ともexit 0。
候補native-workerもexit 0。候補のplatformテストはrelease modeでも5 PASS / 0 FAIL。
各logとhashをこのdirectoryに保持した。差がない結果は既存挙動の互換性確認であり、更新で解消した製品バグとは数えない。

再実行は `npm ci --ignore-scripts`、`node test/helpers/create-test-image.js`、次のcanonical buildと既存テストを用いる。

```sh
SDKROOT=$(xcrun --sdk macosx --show-sdk-path) RUSTUP_TOOLCHAIN=1.88.0 npm run build
SDKROOT=$(xcrun --sdk macosx --show-sdk-path) cargo +1.88.0 test --locked --no-default-features engine::platform::tests -- --nocapture
node test/integration/basic.test.js
node test/integration/from-path-async.test.js
node test/integration/edge-cases.test.js
node test/integration/artifact-compiler.test.js
node test/integration/metadata-options.test.js
node test/integration/error-codes.test.js
node test/integration/native-worker.test.js
```

Hosted CIは最終PR HEADの非tag branchから既存CI.ymlをfull_validation=true / dry_run=trueで実行する。
Linux/macOSのRust TestsでOS境界を直接検証した環境と、6 platform × Node 22/24の候補API/CLI smokeの環境を分けて記録する。
全候補smoke成功は全OSのsyscall直接検証を意味しない。Fuzzは既存11 targetとReportを用い、最終lockfile hash・cache対応・compileログを確認する。
公開済み1.4.2とのbinary hash一致は要求せず、最終コードSHA・依存hash・今回の未公開候補hashを結び付ける。
