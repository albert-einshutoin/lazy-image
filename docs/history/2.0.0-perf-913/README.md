# 2.0.0 three-warning investigation (#913)

Status: investigation in progress; local matched comparison is INCONCLUSIVE. Hosted same-job Linux evidence and the final publication decision are pending. This is not publication approval.

Compare patched security revision a0db8d7f2471c037c10dc2cdd521f2098ea4edae with API-removal revision eafc2dfc48e19ed1af15f1f344583c2ae2d79961, both unpublished 2.0.0. Historical runs 39988612791 and 40117957006 are separate machines with single-shot timers. All output sizes/quality axes match, and sharp timings also increase. CPU model, loaded comparison binary hash, warmup/cache and contention are unknown for those processes. The source runner, fixture, Cargo/npm locks and build configuration are identical. Removed preset() is not on the benchmark path.

The preregistered [protocol](protocol.json) uses the canonical runner, identical instrumentation overlays, isolated builds, sequential AB/BA pairs and every retained sample. The historical baseline and +15% time/+5% size thresholds are unchanged. A first suite follows module/fixture/reference setup; it does not mean OS-cold I/O. Each process then runs three warm suites.

## Local result

Five pairs crossed the prespecified uncertainty boundary, so the one permitted additional five-pair batch ran. At ten pairs, every output hash, dimension, metadata field and existing quality axis matched. No further local measurement is authorized by this protocol.

| Case | Phase | Paired median B/A | Exploratory 95% interval | Status |
|---|---|---:|---|---|
| JPEG | first | 0.9786 | 0.9118 to 1.0286 | pass |
| JPEG | warm | 1.0294 | 0.9851 to 1.1023 | pass |
| AVIF | first | 0.9822 | 0.9530 to 1.0243 | pass |
| AVIF | warm | 0.9896 | 0.9771 to 1.0063 | pass |
| Complex Pipeline | first | 1.0376 | 0.9825 to 1.2747 | inconclusive |
| Complex Pipeline | warm | 1.0000 | 0.9870 to 1.0267 | pass |

The first complex-pipeline interval still crosses 1.15. Its warm median is stable, but the initial sample's uncertainty is retained; no favorable run is selected to declare PASS. These limited observations do not identify the historical VM's contention or prove universal platform performance.

No product/codec/security/API behavior is changed. The only canonical runner change observes output buffers after its existing timers/quality evaluation. The wrapper's metadata and objects are collected outside those timers. A manual opt-in diagnostic is added to the existing benchmark workflow; normal/manual/scheduled benchmark semantics stay intact.
