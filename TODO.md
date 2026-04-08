# Todo List

Remove `src/wizard.rs` from the project, the web setup handles it.. maybe keep for CLI users?

## Frontend

- Rework the Jobs screen sorting/filter island so it uses space more intelligently on narrow screens and overflows in a controlled, intentional-looking way instead of overflowing awkwardly.
- Make the toast across all pages blur the background instead of reading as transparent.
- Fix the Jobs modal so active jobs do not show `Waiting for analysis` while encoding/remuxing is already in progress.
- Reduce the stop/drain redundancy in the header so pressing Stop does not leave both the button and the status pill saying `Stopping`.
- Make the `midnight` OLED theme truly black, without gray treatment or shared gradients.

## Backend

- Investigate why encoding is very slow on macOS even when hardware acceleration is selected.
- Investigate why so many jobs are skipped and why only one job appears to run at a time even when concurrent jobs are enabled.
- Fix the clippy error that is currently blocking CI/CD.

## Jobs / UX

- Improve failed-job explanations on the Jobs screen when the current failure summary is weak or missing.
