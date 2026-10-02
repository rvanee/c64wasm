# ROM images (not in the repository)

Commodore's firmware is copyrighted, so no ROM files are committed here
(`.gitignore` excludes `*.rom`). The web page gets its ROMs from the user;
see the main README.

For the native tests and tools, put your own images here:

- basic.rom    8192 bytes  (BASIC V2, 901226-01)
- kernal.rom   8192 bytes  (KERNAL, 901227-01/02/03)
- chargen.rom  4096 bytes  (character generator, 901225-01)
- 1541.rom     16384 bytes (1541 DOS: 325302-01 + 901229-0x, or a 1541-II 251968-0x)

`node web/tools/rebuild-roms.mjs` rebuilds them here from public listings
(CI does this for each run). Tests that need them skip themselves when
they're missing.
