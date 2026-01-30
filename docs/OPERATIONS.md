# Operation Contracts

This document lists each pipeline operation, its prerequisites, and its side effects.
The goal is to make operation additions safer and to avoid state-management regressions.

## Prerequisite flags

- **decoded_pixels**: Requires decoded pixel buffer (not just metadata).
- **color_state**: Requires color space / bit depth tracking to be available.
- **orientation**: Requires EXIF Orientation metadata (auto-orient).

## Effect flags

- **mutates_pixels**: Writes pixel data.
- **changes_geometry**: Alters dimensions or orientation.
- **normalizes_color**: Normalizes color space / bit depth.

## Operations

| Operation      | Prerequisites                    | Effects                                |
|----------------|----------------------------------|----------------------------------------|
| resize         | decoded_pixels, color_state      | mutates_pixels, changes_geometry       |
| extract        | decoded_pixels, color_state      | mutates_pixels, changes_geometry       |
| crop           | decoded_pixels, color_state      | mutates_pixels, changes_geometry       |
| rotate         | decoded_pixels, color_state      | mutates_pixels, changes_geometry       |
| flipH / flipV  | decoded_pixels, color_state      | mutates_pixels, changes_geometry       |
| brightness     | decoded_pixels, color_state      | mutates_pixels, normalizes_color       |
| contrast       | decoded_pixels, color_state      | mutates_pixels, normalizes_color       |
| autoOrient     | decoded_pixels, color_state, orientation | mutates_pixels, changes_geometry |
| grayscale      | decoded_pixels, color_state      | mutates_pixels, changes_geometry, normalizes_color |
| colorSpace     | decoded_pixels, color_state      | mutates_pixels, normalizes_color       |

## How it is enforced

- Each `Operation` variant has a **contract** (`Operation::contract()`) that declares its prerequisites and effects.
- `validate_operation_sequence` checks the contract list before execution. A missing prerequisite returns `InvalidArgument`.
- Adding a new operation requires defining its contract via a `match` expression, so omissions fail at compile time.

## Future extensions

- If an operation ever requires additional state (e.g., ICC presence, alpha-premultiplication), add a new prerequisite flag and extend the validator.
- Keep this table in sync with the contract `match` in `ops.rs`.
