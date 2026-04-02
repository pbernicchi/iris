debug notes

0xffffffff9fc31538 the 1000 instructions timing loop
ttyinput function
0xffffffff9fc3164c 27 bd ff d8     addiu      sp,sp,-0x28

consgetc 0xffffffff9fc20024
    actual read 0xffffffff9fc201e0
ttyinput 0xffffffff9fc3164c
ttypoll 0xffffffff9fc1fc84
circ_putc 0xffffffff9fc31888


Based on my analysis of the IRIX source code, here are the best breakpoint locations for debugging SCSI device discovery and hwgraph creation:

Controller Registration & Initialization
wd93_earlyinit - Early initialization of WD93 controller, called before main()
wd93edtinit - EDT (Equipped Device Table) initialization, sets up controller structures
wd93_resetbus - SCSI bus reset, happens during initialization
Device Discovery & Inquiry
wd93_inq (line 977 in wd93.c) - KEY FUNCTION - Performs INQUIRY command and caches result

Check if info->si_inq is being populated
Check if line 1038 condition passes: sp->sr_status == SC_GOOD && sp->sr_scsi_status == ST_GOOD
Check if scsi_device_update() is called at line 1081
wd93intr (line 1093) - Interrupt handler, processes command completions

Verify sr_scsi_status gets set correctly at line 2525
handle_intr (line 2409) - Main interrupt processing

Check state == ST_SATOK (0x16) for successful completion
Verify TARGET_LUN register value gets stored to sr_scsi_status
Hwgraph Device Registration
scsi_device_update (line 1990 in scsi.c) - CRITICAL - Adds device to hwgraph

This is called from wd93_inq line 1081
Creates /hw/scsi entries
Verify lun_vhdl is valid
Check inv[0] peripheral qualifier bits
scsi_dev_add (called from scsi_device_update line 2031) - Creates scsi device vertex in hwgraph

dkscopen (line 480 in dksc.c) - KEY FOR ROOT MOUNT - Opens disk device during boot

Line 527-534: Calls SDI_INQ() to get cached inquiry data
Will fail with ENODEV if inquiry data not cached
Volume Header & Partition Discovery
dkscioctl - Handles disk ioctls including reading volume header
dksc_part_setup - Creates partition entries in hwgraph (e.g., /hw/scsi/0/1/0/vh, /hw/scsi/0/1/0/partition/7)
Boot-time Root Device
devinit (line 1877 in startup.c) - MAIN BOOT FUNCTION - Initializes root device
Calls devavail() to check device availability
devavail (line 2151 in startup.c) - Tries to open root device path
baddev - Prints "Root device /hw/scsi not available" error
Recommended Debugging Sequence
Phase 1: Controller Initialization

￼
break wd93_earlyinit
break wd93edtinit
Phase 2: Device Discovery

￼
break wd93_inq
break scsi_device_update
At wd93_inq, check:

Is INQUIRY command being sent?
Does sp->sr_status == 0 (SC_GOOD)?
Does sp->sr_scsi_status == 0 (ST_GOOD)?
Is scsi_device_update() being called?
Phase 3: Hwgraph Registration

￼
break scsi_dev_add
Verify device vertices are created in hwgraph.

Phase 4: Root Device Mount

￼
break devinit
break devavail
break dkscopen
At dkscopen, check:

Line 527: Does SDI_INQ() return cached inquiry data?
If NULL, inquiry was never cached successfully
Critical Values to Watch:

sp->sr_status - Should be 0 (SC_GOOD) for success
sp->sr_scsi_status - Should be 0 (ST_GOOD) for success
info->si_inq in wd93_inq - Should be populated with inquiry data
ret[0] in wd93_inq line 1080 - Should NOT have bit 5 set (removable)
lun_vhdl - Should be valid vertex handle
The most likely issue based on our hardware logs is that even though INQUIRY succeeds at the hardware level, something in the software path prevents scsi_device_update() from being called or the inquiry data from being cached in wd93_info[adap][targ][lun].si_inq.

  2. Early Boot / Low-Level Console Output:
      The most interesting usage for us is the global dprintf. We see it declared as extern and used in many core ml (machine-level) files:
       * irix/kern/ml/IP22.c: dprintf(fmt,ARGS)
       * irix/kern/ml/IP26.c: dprintf(fmt,ARGS)
       * irix/kern/ml/EVEREST/init.c: dprintf(fmt,ARGS)
       * irix/kern/os/printf.c: extern void dprintf(char *, ...);

      The definition in irix/kern/ml/IP22.c reveals how it works:

    1     void
    2     dprintf(fmt,ARGS)
    3     char *fmt;
    4     long ARGS;
    5     {
    6         /*
    7          * Check for presence of symmon
    8          */
    9         if ( SPB->DebugBlock && ((db_t *)SPB->DebugBlock)->db_printf ) {
   10             (*((db_t *)SPB->DebugBlock)->db_printf)(fmt, ARGS);
   11         } else {
   12             /*
   13              * cn_is_inited() implies that PROM bss has been wiped out
   14              */
   15             if (cn_is_inited())
   16                 cmn_err(CE_CONT,fmt,ARGS);
   17             else {
   18                 /*
   19                  * try printing through the prom
   20                  */
   21                 arcs_printf (fmt,ARGS);
   22             }
   23         }
   24     }
      This dprintf is a wrapper that intelligently decides where to send output:
       * If symmon (the SGI kernel debugger) is loaded, it uses its printf.
       * If the kernel console (cn) is initialized, it uses cmn_err.
       * If neither is ready (i.e., very early in boot), it calls directly into the PROM's `arcs_printf` function.

      This dprintf isn't controlled by a variable; it's a direct, low-level print function. It's used for critical errors like panics. It doesn't provide extra verbosity itself, but it's the
  mechanism used by other verbose flags.

  Final List of Verbosity Variables

  Based on all our analysis, here is the definitive list of variables you can set in the PROM to get maximum verbosity for debugging device registration and the hardware graph.

   1. `showconfig`: This is the most important one. It is the master switch for verbose logging during device probing and initialization. Nearly every driver checks this flag.
       * How to set: setenv showconfig 1

   2. `diagmode`: We saw this in stand/arcs/ide/fforward/scsi/scsi_diag.c. Setting this to v (verbose) enables more detailed SCSI diagnostics in the IDE and standalone environment. While
      it's mainly for the ide, the kernel drivers may also check it.
       * How to set: setenv diagmode v

   3. `kdebug`: This is a more general kernel debugging flag. We saw in irix/kern/ml/IP22.c that it's checked. Setting this can enable a variety of debugging features, sometimes including
      more verbose output.
       * How to set: setenv kdebug 1

   4. Driver-Specific Variables: Some drivers have their own debug flags that can be set from the PROM. These are less common but can be very useful. Examples from our grep results (though
      some are for different platforms):
       * adp_verbose=1 (for the adp78 SCSI driver)
       * pcimh_verbose=1 (for the PCMCIA host adapter)
       * plp_debug=1 (for the parallel port)
      You would set these just like the others: setenv adp_verbose 1.

  Recommended Settings for Maximum Verbosity

  To get the most information possible during kernel boot, especially around device and hwgraph initialization, you should set the following variables in the PROM environment before booting
  the kernel:

   1 setenv showconfig 1
   2 setenv diagmode v
   3 setenv kdebug 1

  This combination will turn on:
   * The global showconfig flag, making most drivers print what they are doing.
   * The verbose diagnostic mode, which affects SCSI and other low-level probing.
   * General kernel debugging flags.

  This should give you a flood of information on the console, allowing you to see exactly where devices are being probed, what INQUIRY data is being returned, and how the hwgraph is being
  constructed. This is your best tool for debugging why a device isn't being recognized correctly.

## CPU Execution Control

### `stop` Command Behavior

The `stop` command is used to halt CPU execution and works in two distinct modes:

1.  **Background Mode (Threaded)**
    *   **Initiated by:** `cpu start`
    *   **Execution:** The CPU runs in a dedicated background thread.
    *   **Stop Behavior:** The `stop` command sets the global `running` flag to `false` and synchronously waits (`join`) for the background thread to finish its current instruction block and terminate.
    *   **Result:** The command returns only after the CPU has fully stopped.

2.  **Debug Mode (Threaded)**
    *   **Initiated by:** `run`, `continue`, `step`, `next`, `finish`
    *   **Execution:** `run_debug_loop` runs in its own dedicated thread, equivalent to `start`. It does not require cloning the executor.
    *   **Async vs Sync:**
        *   `run`, `continue`: Call `run_debug_loop` asynchronously.
        *   `step`, `next`: Call `run_debug_loop` asynchronously but wait for it to finish (`join`).
    *   **Output:** `run_debug_loop` must not use `print!`. Output is either returned immediately before the thread starts or collected and returned to the monitor user when the thread finishes.
    *   **Stop Behavior:** The `stop` command sets the global `running` flag to `false`.

  This should give you a flood of information on the console, allowing you to see exactly where devices are being probed, what INQUIRY data is being returned, and how the hwgraph is being
  constructed. This is your best tool for debugging why a device isn't being recognized correctly.
