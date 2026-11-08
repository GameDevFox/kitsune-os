# Credits

## Art and voice

<img alt="Aki" src="./kernel/image/aki/base.png" width="150px">

**Aki**, the character both kernels draw, was commissioned from **ShizukoDesu**: https://shizukodesu.carrd.co/

Her pictures are in `kernel/image/aki/`.

**Momo's voice** was commissioned from **MomoBunnyVA**. Her recordings aren't in this repo: `next/tools/voice.py` plays a clip from a recording kept on the machine that builds the kernel.

<img alt="Mascot" src="./kernel/image/mascot.png" width="150px">

**The mascot** in `kernel/image/mascot.png` is by **Glorin Art** ("GloristicArt" on Etsy): https://www.etsy.com/shop/GloristicArt

These works are credited here, and they aren't covered by the code's license in `LICENSE`. Please don't reuse them without the artists' permission.

## Code and references

- **The GPU cube** (`next/kernel/src/board/raspi4/gpu/`) uses shader code and command layouts from Random06457's [rpi4-gpu-bare-metal-examples](https://github.com/Random06457/rpi4-gpu-bare-metal-examples), under the MIT license, whose notice is in that folder's `CREDITS`.
- **The square's rainbow** (`next/scripts/make-rainbow.py`) picks its colors in OKLab, using the conversion published by Björn Ottosson.
- **Register layouts and constants** come from the Raspberry Pi Linux kernel's drivers, Mesa's `v3d_packet.xml`, and Broadcom's BCM2711 documentation, which the code cites where it uses them.
