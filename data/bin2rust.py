import argparse
import os
import sys

def main():
    parser = argparse.ArgumentParser(
        description="Convert a binary file (like a VGA font) to a Rust static array."
    )
    parser.add_argument("input_file", help="Path to the binary file (e.g., VGA8.F16)")
    parser.add_argument("-o", "--output", help="Path to the output Rust file", default="font.rs")
    parser.add_argument("-n", "--name", help="Name of the static variable", default="FONT_DATA")
    
    args = parser.parse_args()
    
    input_path = args.input_file
    output_path = args.output
    var_name = args.name.upper()

    if not os.path.exists(input_path):
        print(f"Error: Input file '{input_path}' not found.")
        sys.exit(1)

    try:
        with open(input_path, "rb") as f:
            data = f.read()
    except IOError as e:
        print(f"Error reading file: {e}")
        sys.exit(1)

    file_size = len(data)
    print(f"Read {file_size} bytes from {input_path}...")

    try:
        with open(output_path, "w") as out:
            out.write(f"// Generated from {input_path}\n")
            out.write(f"// Size: {file_size} bytes\n\n")
            
            # Define the static array
            out.write(f"pub static {var_name}: [u8; {file_size}] = [\n")
            
            # Write bytes formatted as hex, 16 per line
            bytes_per_row = 16
            for i in range(0, file_size, bytes_per_row):
                chunk = data[i:i + bytes_per_row]
                hex_strings = [f"0x{b:02X}" for b in chunk]
                line_content = ", ".join(hex_strings)
                out.write(f"    {line_content},\n")
            
            out.write("];\n")
            
        print(f"Successfully wrote Rust source to {output_path}")

    except IOError as e:
        print(f"Error writing output file: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
