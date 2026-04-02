# HAL2 Audio ASIC — Register Reference and Emulation Notes

The HAL2 (High-bandwidth Audio/Video Link 2) is the audio ASIC in the SGI Indy.
It is controlled entirely through **indirect registers**: values are staged in one or
more Indirect Data Registers (`IDR0–IDR3`) and then committed by writing the target
register address to the Indirect Address Register (`IAR`).

---

## Physical Register Map

| Offset | Name   | Description                         |
|--------|--------|-------------------------------------|
| `0x10` | `ISR`  | Interrupt / global control register |
| `0x20` | `REV`  | Chip revision (read-only, → `0x4010`) |
| `0x30` | `IAR`  | Indirect address register           |
| `0x40` | `IDR0` | Indirect data register 0            |
| `0x50` | `IDR1` | Indirect data register 1            |
| `0x60` | `IDR2` | Indirect data register 2            |
| `0x70` | `IDR3` | Indirect data register 3            |

Base physical address: `0x1FBD8000` (Indy).  All accesses are 32-bit.

---

## ISR — Interrupt Status Register (`0x10`)

Bit | Name                | Active | Effect
----|---------------------|--------|-----------------------------------------------------------
 2  | `CODEC_MODE`        | high   | 0 = Indigo mode (default); 1 = Quad mode (two independent stereo pairs)
 3  | `GLOBAL_RESET_N`    | low    | Writing 0 resets the entire HAL2 chip
 4  | `CODEC_RESET_N`     | low    | Writing 0 resets only the codec/synth section

**Reset sequence** (from `SetupHAL2` in a2diags.c):
```
ISR ← 0x00          (assert global reset)
~1 ms delay
ISR ← 0x18          (release global + codec reset)
~70 µs delay
```

---

## IAR — Indirect Address Register (`0x30`)

The IAR word encodes both the target register group and the read/write direction.

```
Bit 15:    1 = read from indirect register → IDR,  0 = write IDR → indirect register
Bits 14:12 + 11:8: register group and index (see table below)
Bits 3:2:  parameter selector within the group (1 = control word 1, 2 = control word 2)
```

### Indirect Register Addresses

| IAR value  | Symbol                  | IDR words used | Description                         |
|------------|-------------------------|----------------|-------------------------------------|
| `0x9104`   | `HAL2_DMA_ENABLE_W`     | IDR0           | DMA enable bitmask (write)          |
| `0x9904`   | `HAL2_DMA_ENABLE_R`     | IDR0           | DMA enable bitmask (read-back)      |
| `0x910C`   | `HAL2_DMA_DRIVE_W`      | IDR0           | DMA drive bitmask (write)           |
| `0x990C`   | `HAL2_DMA_DRIVE_R`      | IDR0           | DMA drive bitmask (read-back)       |
| `0x9108`   | `HAL2_DMA_ENDIAN_W`     | IDR0           | DMA endian (0 = big-endian)         |
| `0x1404`   | `HAL2_CODECA_CTRL1_W`   | IDR0           | Codec A channel / clock / mode      |
| `0x1408`   | `HAL2_CODECA_CTRL2_W`   | IDR0,IDR1      | Codec A gain / attenuation / mute   |
| `0x1504`   | `HAL2_CODECB_CTRL1_W`   | IDR0           | Codec B channel / clock / mode      |
| `0x1508`   | `HAL2_CODECB_CTRL2_W`   | IDR0,IDR1      | Codec B gain / attenuation / mute   |
| `0x0304`   | `HAL2_AESTX_CTRL_W`     | IDR0           | AES TX channel / clock / mode       |
| `0x0204`   | `HAL2_AESRX_CTRL_W`     | IDR0           | AES RX channel / clock              |
| `0x2104`   | `HAL2_BRES1_CTRL1_W`    | IDR0           | BRES 1 master clock select          |
| `0x2108`   | `HAL2_BRES1_CTRL2_W`    | IDR0,IDR1      | BRES 1 inc / modctrl                |
| `0x2204`   | `HAL2_BRES2_CTRL1_W`    | IDR0           | BRES 2 master clock select          |
| `0x2208`   | `HAL2_BRES2_CTRL2_W`    | IDR0,IDR1      | BRES 2 inc / modctrl                |
| `0x2304`   | `HAL2_BRES3_CTRL1_W`    | IDR0           | BRES 3 master clock select          |
| `0x2308`   | `HAL2_BRES3_CTRL2_W`    | IDR0,IDR1      | BRES 3 inc / modctrl                |
| `0x1504`   | `HAL2_RELAY_CONTROL_W`  | IDR0           | Headphone relay (1 = headphone out) |

---

## Bresenham Clock Generators

HAL2 has three identical Bresenham clock generators (`BRES1`, `BRES2`, `BRES3`).
Each produces an audio sample-rate clock from a fixed master oscillator.

### CTRL1 — Master clock select (IDR0)

Value | Master clock
------|-------------
`0`   | 48 000 Hz
`1`   | 44 100 Hz
`2`   | AES RX recovered clock (used for digital sync)

### CTRL2 — Frequency divider (IDR0 = inc, IDR1 = modctrl)

```
Output frequency = master_clock × inc / mod

modctrl = (inc − mod − 1) & 0xFFFF   ← what is actually written to IDR1
```

Common settings (48 kHz master):

| Rate   | inc | mod | modctrl       |
|--------|-----|-----|---------------|
| 48000  | 4   | 4   | `0xFFFF` (−1) |
| 32000  | 4   | 6   | `0xFFF9`      |
| 16000  | 4   | 12  | `0xFFF3`      |
| 8000   | 4   | 24  | `0xFFE7`      |

Common setting (44.1 kHz master):

| Rate   | inc | mod | modctrl       |
|--------|-----|-----|---------------|
| 44100  | 1   | 1   | `0xFFFF` (−1) |

**Note:** the diags use `inc=4, mod=4` (not `inc=1, mod=1`) as their 48 kHz default,
giving `modctrl = 4 − 4 − 1 = −1 = 0xFFFF`.  Both produce the same ratio.

---

## DMA Enable / Drive Registers

### `HAL2_DMA_ENABLE_W` (`0x9104`)

One-bit-per-engine enable mask written to IDR0:

Bit | Engine
----|--------
 1  | AES RX
 2  | AES TX (⚠ 0x04, not 0x02; AESTX is bit 2)
 3  | Codec A (0x08)
 4  | Codec B (0x10)

The diags **read-modify-write** this register (`HAL2_DMA_ENABLE_R` then `HAL2_DMA_ENABLE_W`)
so bits for other engines are preserved.  The emulator must store this register and
return it on IAR reads.

### `HAL2_DMA_DRIVE_W` (`0x910C`)

One-bit-per-HPC3-channel mask written to IDR0.  Bit N = drive enable for HPC3 PDMA
channel N.  Also read-modified-written by the driver.

---

## Codec Control Register 1 — Channel, Clock, Mode

Written to IDR0 then strobed via `HAL2_CODECA_CTRL1_W` / `HAL2_CODECB_CTRL1_W`.

```
Bits  2:0  — HPC3 PDMA channel number (0–7) that feeds this codec
Bits  4:3  — Bresenham clock ID: 0=BRES1, 1=BRES2, 2=BRES3
Bits  9:8  — Channel mode (see below)
Bit  10    — Timestamp enable (used by clock-test diagnostics)
```

### Channel mode field (bits 9:8)

Value | Mode   | Channels consumed per sample period | Notes
------|--------|-------------------------------------|------
`0`   | —      | (undefined / reset state)           |
`1`   | Mono   | 1 × 32-bit word                     | HAL2 duplicates it to both L and R outputs
`2`   | Stereo | 2 × 32-bit words: [L, R]            | Most common mode
`3`   | Quad   | **Codec A only**; see below         |

**Quad mode (CHANNEL_MODE = 3) — two flavours:**

1. **Single-stream quad** (`ISR.CODEC_MODE = 0`): Codec A receives one DMA stream of
   4-word frames `[FL, FR, BL, BR]`.  HAL2 sends FL/FR to Codec A DACs and BL/BR
   to Codec B DACs internally.  Only one HPC3 DMA channel is needed.

2. **Dual-stream quad** (`ISR.CODEC_MODE = 1`): Codec A and Codec B are independent
   stereo devices each with their own DMA channel (set via CTRL1 on each codec
   separately).  This is what `TestLine(QUAD)` / `thru -q` use.  In this mode the
   `SetupDMAChannel` code sets Codec A to stereo on channel N and Codec B to stereo
   on channel N+1; both codecs run simultaneously.

---

## Audio Sample Format

HAL2 DMA transfers 32-bit words.  The audio data is **24-bit signed PCM, left-justified**:

```
 31          8 7        0
 [ 24-bit PCM | 00000000 ]
```

In the diagnostics this is constructed as `sintab_value << 8`.
The HPC3 PDMA layer on the Indy delivers **16-bit values** to the HAL2 (it strips the
low 8 bits of each 32-bit word), so the emulator's `DmaClient::read()` already
returns the upper 16 bits ready for use as `i16`.

---

## Memory Layout by Channel Mode

### Mono (mode = 1)
```
Word:  [  M0  ]  [  M1  ]  [  M2  ]  ...
```
Each 32-bit word is a single mono sample.  HAL2 feeds it to both Left and Right DACs.

### Stereo (mode = 2)
```
Word:  [  L0  ]  [  R0  ]  [  L1  ]  [  R1  ]  ...
```
Interleaved left/right pairs.  Most programs and the default `thru` mode use this.

### Quad — single-stream (Codec A, mode = 3, ISR.CODEC_MODE = 0)
```
Word:  [ FL0 ]  [ FR0 ]  [ BL0 ]  [ BR0 ]  [ FL1 ]  ...
```
HAL2 routes FL/FR to Codec A and BL/BR to Codec B.

### Dual-stream quad (ISR.CODEC_MODE = 1)
```
Codec A DMA:  [  L0  ]  [  R0  ]  [  L1  ]  [  R1  ]  ...  (stereo, mode=2)
Codec B DMA:  [  L0  ]  [  R0  ]  [  L1  ]  [  R1  ]  ...  (stereo, mode=2)
```
Two completely independent DMA streams, one per codec.

---

## Codec Control Register 2 — Gain, Attenuation, Mute

Written as IDR0 = ctrl2_word_0, IDR1 = ctrl2_word_1.

```
IDR0 bits 7:4  — Left channel input gain  (0–15, 4 bits)
IDR0 bits 3:0  — Right channel input gain (0–15, 4 bits)
IDR0 bits 9:8  — Input source select (0 = line1, 3 = line2/mic)

IDR1 bits 11:7 — Left D/A output attenuation  (0 = no atten, 31 = max)
IDR1 bits 6:2  — Right D/A output attenuation
IDR1 bits 1:0  — Mic select / headphone power
                 0x03 = line input, unmuted
                 0x01 = mic input (with mic power)
                 0x00 = mic input × 10 gain
```

The emulator does not need to implement gain/attenuation — these affect only the
real analog signal chain, not the digital sample values.

---

## AES TX Control Register

Written as a single IDR0 value, same bitfield layout as Codec CTRL1:

```
Bits 2:0 — HPC3 DMA channel
Bits 4:3 — Bresenham clock ID
Bits 9:8 — Channel mode (2 = stereo, always used in practice)
```

Example from Test_Rx_Clock: `IDR0 = 0x213` → channel 3, BRES clock 2, stereo.

---

## Emulator Implementation Notes

### What is correctly implemented

- ISR global/codec reset handling
- IAR decode for codec A/B, AES TX/RX, BRES 1–3, global DMA enable/drive
- Bresenham clock rate computation (master × inc / mod)
- Per-codec cpal output stream opened at the exact configured sample rate (no resampling)
- DMA burst of 4 stereo frames per loop iteration
- Ring buffer large enough to absorb OS scheduling jitter
- Mono (mode=1) and stereo (mode=2) sample demux
- Quad single-stream (mode=3) — rear channels consumed and discarded
- Codec B input — silence written back at configured rate

### Known gaps / not implemented

- **`HAL2_DMA_ENABLE_R` / `HAL2_DMA_DRIVE_R` read-back** — drivers OR new bits into
  the existing value.  If they read back 0 they will still OR in their bits so this
  is unlikely to cause failures in practice, but is not strictly correct.
- **`HAL2_DMA_ENDIAN_W`** — always treated as big-endian (correct default).
- **`HAL2_RELAY_CONTROL_W`** — headphone relay, no analog hardware to emulate.
- **Dual-stream quad mode** (`ISR.CODEC_MODE = 1`) — both Codec A and Codec B would
  need independent output streams running simultaneously.  Currently only Codec A's
  stream is opened; Codec B input is drained to silence.
- **AES TX/RX clock locking** — AESRX recovered clock as BRES master (mode `0x2` in
  CTRL1) is not implemented; the emulator falls back to 44100 Hz.
- **Codec CTRL2 (gain/atten/mute)** — not emulated; all analog processing is bypassed.
- **Timestamp mode** (bit 10 of codec CTRL1) — used by the clock diagnostic test to
  interleave timing data into the DMA buffer; not implemented.
