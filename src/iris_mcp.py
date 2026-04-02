import asyncio
import sys
from mcp.server.fastmcp import FastMCP

# Initialize FastMCP server
mcp = FastMCP("iris-monitor")

class IrisClient:
    def __init__(self, host='127.0.0.1', port=8888):
        self.host = host
        self.port = port
        self.reader = None
        self.writer = None
        self._lock = asyncio.Lock()

    def _reset(self):
        if self.writer:
            try:
                self.writer.close()
            except Exception:
                pass
        self.reader = None
        self.writer = None

    async def connect(self, retries=10, delay=2.0):
        for attempt in range(retries):
            try:
                self.reader, self.writer = await asyncio.open_connection(self.host, self.port)
                # Read initial banner
                await self.read_until_prompt()
                print(f"Connected to IRIS monitor at {self.host}:{self.port}", file=sys.stderr)
                return
            except Exception as e:
                self._reset()
                print(f"IRIS connect attempt {attempt+1}/{retries} failed: {e}", file=sys.stderr)
                if attempt < retries - 1:
                    await asyncio.sleep(delay)
        raise ConnectionError(f"Could not connect to IRIS monitor at {self.host}:{self.port} after {retries} attempts")

    async def read_until_prompt(self):
        data = b""
        while True:
            chunk = await self.reader.read(4096)
            if not chunk:
                raise ConnectionError("IRIS monitor closed connection")
            data += chunk
            if data.endswith(b"> "):
                return data[:-2].decode('utf-8', errors='replace').strip()

    async def send_command(self, cmd):
        async with self._lock:
            for attempt in range(2):
                try:
                    if not self.writer or self.writer.is_closing():
                        await self.connect()
                    self.writer.write(f"{cmd}\n".encode())
                    await self.writer.drain()
                    return await self.read_until_prompt()
                except Exception as e:
                    print(f"IRIS command failed (attempt {attempt+1}): {e}", file=sys.stderr)
                    self._reset()
                    if attempt == 0:
                        await self.connect()
            raise ConnectionError("Failed to send command after reconnect")

client = IrisClient()

@mcp.tool()
async def run_command(command: str) -> str:
    """Run a raw command on the IRIS monitor.
    
    Use this for commands not explicitly exposed as tools.
    Common commands: 'tlb dump', 'u' (undo), 'bp list'.
    """
    return await client.send_command(command)

@mcp.tool()
async def read_memory(address: str, count: int = 1) -> str:
    """Read memory words from IRIS.
    
    Args:
        address: Hex address (e.g., '0x80001000')
        count: Number of words to read
    """
    return await client.send_command(f"mem {address} {count}")

@mcp.tool()
async def read_string(address: str, max_len: int = 256) -> str:
    """Read a null-terminated string from memory."""
    return await client.send_command(f"ms {address} {max_len}")

@mcp.tool()
async def write_memory(address: str, value: str, size: str = "w") -> str:
    """Write value to memory.
    
    Args:
        address: Hex address
        value: Value to write (hex or dec)
        size: Size 'b' (byte), 'h' (half), 'w' (word), 'd' (double)
    """
    return await client.send_command(f"mw {address} {value} {size}")

@mcp.tool()
async def step(count: int = 1) -> str:
    """Step execution by N instructions. Blocks until complete."""
    return await client.send_command(f"step block {count}")

@mcp.tool()
async def next_instruction(count: int = 1) -> str:
    """Step over function calls (next). Blocks until complete."""
    return await client.send_command(f"next block {count}")

@mcp.tool()
async def get_registers() -> str:
    """Dump general purpose registers."""
    return await client.send_command("regs")

@mcp.tool()
async def disassemble(address: str, count: int = 10) -> str:
    """Disassemble instructions.
    
    Args:
        address: Start address (hex) or register name (e.g. 'pc')
        count: Number of instructions
    """
    return await client.send_command(f"dis {address} {count}")

@mcp.tool()
async def backtrace(frames: int = 10) -> str:
    """Show stack backtrace."""
    return await client.send_command(f"bt {frames}")

@mcp.tool()
async def add_breakpoint(address: str, kind: str = "pc") -> str:
    """Add a breakpoint.
    
    Args:
        address: Address to break at (hex or symbol)
        kind: Type of breakpoint: 'pc' (execution), 'read', 'write', 'access'
    """
    return await client.send_command(f"bp add {address} {kind}")

@mcp.tool()
async def remove_breakpoint(id: int) -> str:
    """Remove a breakpoint by ID."""
    return await client.send_command(f"bp del {id}")

@mcp.tool()
async def list_breakpoints() -> str:
    """List all breakpoints."""
    return await client.send_command("bp list")

@mcp.tool()
async def get_status() -> str:
    """Get CPU running status and current PC."""
    return await client.send_command("status")

@mcp.tool()
async def continue_execution(until: str = None) -> str:
    """Continue execution (run). Blocks until a breakpoint is hit or execution stops.

    Args:
        until: Optional address to run until (temporary breakpoint)
    """
    if until:
        return await client.send_command(f"run block {until}")
    return await client.send_command("run block")

@mcp.tool()
async def finish_function() -> str:
    """Run until the current function returns. Blocks until complete."""
    return await client.send_command("finish block")

@mcp.tool()
async def read_cop0() -> str:
    """Dump CP0 registers."""
    return await client.send_command("cop0")

@mcp.tool()
async def read_cop1() -> str:
    """Dump CP1 (FPU) registers."""
    return await client.send_command("cop1")

@mcp.tool()
async def translate_address(address: str) -> str:
    """Translate virtual address to physical."""
    return await client.send_command(f"translate {address}")

@mcp.tool()
async def dump_tlb() -> str:
    """Dump TLB entries."""
    return await client.send_command("tlb dump")

@mcp.tool()
async def lookup_symbol(name_or_addr: str) -> str:
    """Lookup symbol by name or address."""
    return await client.send_command(f"sym {name_or_addr}")

@mcp.tool()
async def undo(count: int = 1) -> str:
    """Undo the last N instructions."""
    return await client.send_command(f"undo {count}")

@mcp.tool()
async def traceback(count: int = 10) -> str:
    """Show execution traceback (history of executed instructions)."""
    return await client.send_command(f"dt {count}")

if __name__ == "__main__":
    mcp.run()
