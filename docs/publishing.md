# Publishing c64wasm on GitHub

These steps are for Windows. They start from `C:\Users\renev\develop\c64wasm`.
Replace `<you>` with your GitHub user name.

## 1. Check it locally

Open a Command Prompt in the project folder and run:

```bat
cd C:\Users\renev\develop\c64wasm
cargo fmt --all --check
cargo clippy --release --workspace --all-targets -- -D warnings
cargo test --release --workspace
build_wasm.cmd
python serve.py
```

The CI runs the first three commands too, so if they pass here, the build
on GitHub will pass. `build_wasm.cmd` builds the WebAssembly module into
`web\pkg`.

If you have Node.js, also run the page's tests:

```bat
cd web
node --test
cd ..
```

Then open http://localhost:8000/ in an InPrivate window. It has no stored
ROMs, so it shows what a new visitor sees:

1. The **Commodore ROMs needed** dialog opens.
2. Click **Find them automatically**. All four ROMs should get a ✓.
3. Click **Switch on**. Then click **test disk** under *Disk*, type
   `LOAD"$",8` and `LIST`.
4. Try the desk:
   - hover over the little display on the 1541;
   - click the door under the monitor's screen, and grab and turn a knob;
   - click the monitor's power button and watch the red LED;
   - in the settings, under *Display*, switch to the flat panel and zoom in.
5. Under *Settings → ROMs*, add a ROM by URL. For example, add the 1541
   ROM from
   `https://retroisle.com/commodore/c64128/Technical/Firmware/1541romdisassembly.php`.
   That site doesn't allow downloading, so the page asks you to paste it:
   1. Open the address in a new tab.
   2. Press Ctrl+A, then Ctrl+C.
   3. Paste with Ctrl+V into the box.

## 2. Create the repository

1. Go to https://github.com/new.
2. Repository name: `c64wasm`. Choose **Public**: GitHub Pages is free for
   public repositories.
3. Don't add a README, .gitignore or licence; they are already in the
   folder.
4. Click **Create repository**.
5. In the new repository, open **Settings → Pages**. Under *Build and
   deployment*, set **Source** to **GitHub Actions**.

## 3. Upload the code

You need Git (https://git-scm.com). Run these once to set your name and
GitHub's no-reply address, which keeps your e-mail address private. It's
under GitHub *Settings → Emails*.

```bat
git config --global user.name "R.F. van Ee"
git config --global user.email "<id>+<you>@users.noreply.github.com"
```

Then, in the project folder:

```bat
git init -b main
git add .
git status
```

Check the list `git status` prints. It must not contain any of these:

- `.rom` files;
- anything under `target\`, `web\pkg\` or `test-data\`.

`.gitignore` excludes them. Then:

```bat
git commit -m "c64wasm: a cycle-exact C64 and 1541 in the browser"
git remote add origin https://github.com/<you>/c64wasm.git
git push -u origin main
```

The first push opens a browser window to sign in to GitHub.

GitHub Desktop works too:

1. Choose *File → Add local repository* and pick the folder. Let it create
   the repository when asked.
2. Commit.
3. Choose *Publish repository* and untick "Keep this code private".

## 4. Watch the pipeline

Open the repository's **Actions** tab. The *CI* run has three jobs:

1. **Rust tests**: formatting and clippy. Then it rebuilds the Commodore
   ROMs from public listings into `roms/`, for this run only, and runs
   `cargo test` and the page's Node tests. The run's summary page shows which
   ROMs were rebuilt. If a listing site is down, that ROM is missing, a
   warning appears, and the tests that need it skip themselves.
2. **Build the web page**: compiles the WebAssembly module and checks the
   JavaScript. The finished page is under *Artifacts* on the run's page as
   `c64wasm-web.zip`.
3. **Publish to GitHub Pages**: runs on pushes to `main` only.

When it's green, the page is at `https://<you>.github.io/c64wasm/`. The
link is also on the run's page and under *Settings → Pages*. Open it in an
InPrivate window and repeat the checks from step 1.

If *Publish* fails with a permissions message, check step 2.5 (Source =
GitHub Actions). Then click **Re-run failed jobs**.

## 5. Finishing touches

On the repository's main page, click the gear next to **About**:

- **Description**: *A cycle-exact Commodore 64 and 1541 in the browser
  (Rust + WebAssembly)*.
- **Website**: tick *Use your GitHub Pages website*.
- **Topics**: `c64`, `commodore-64`, `emulator`, `rust`, `webassembly`,
  `1541`.

In `crates/c64-core/Cargo.toml`, you can add this line under `[package]`
and commit it:

```toml
repository = "https://github.com/<you>/c64wasm"
```

## 6. Later

- **A new version.** Commit and push. Tests run on every push and pull
  request; the site updates on pushes to `main`.
- **A release with a downloadable build.** Run these, and the zip of the
  built page is attached to a GitHub release:

  ```bat
  git tag v0.1
  git push origin v0.1
  ```
- **Privacy.** `web/assets/setup.jpg` (the photo of the desk) becomes public
  with the page.
- **ROMs.** Never commit ROM files. The licence (MIT or Apache-2.0) covers
  your code only, not Commodore's firmware.
