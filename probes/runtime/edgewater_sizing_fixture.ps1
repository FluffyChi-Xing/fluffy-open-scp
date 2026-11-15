param([Parameter(Mandatory=$true)][string]$Manifest)
$ErrorActionPreference = 'Stop'
if ([IntPtr]::Size -ne 4) { throw 'Run with 32-bit PowerShell' }
$sizing = Get-Content -LiteralPath $Manifest -Raw | ConvertFrom-Json
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

public static class EdgewaterSizingFixture {
    [DllImport("kernel32", SetLastError=true)]
    static extern IntPtr VirtualAlloc(IntPtr address, UIntPtr size, uint kind, uint protection);
    [DllImport("kernel32", SetLastError=true)]
    static extern bool VirtualProtect(IntPtr address, UIntPtr size, uint protection, out uint old);
    [DllImport("kernel32", SetLastError=true)]
    static extern bool VirtualFree(IntPtr address, UIntPtr size, uint kind);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    delegate void Run(IntPtr instance, IntPtr source);

    static byte[] Hex(string input) {
        var result = new byte[input.Length / 2];
        for (int i = 0; i < result.Length; i++)
            result[i] = Convert.ToByte(input.Substring(i * 2, 2), 16);
        return result;
    }

    public static void Test(string body, string name, uint environment) {
        // Preserve nonvolatile registers and reproduce the displaced code's input context.
        // For the bounds stub mimic the three outstanding stack arguments at entry.
        string prefixCode;
        string suffixCode;
        bool stamp = name.StartsWith("stamp", StringComparison.Ordinal);
        bool control = name.StartsWith("controlExtent", StringComparison.Ordinal);
        switch (name) {
            case "grid": prefixCode = "56578b74240c8b442410"; suffixCode = "5f5ec3"; break;
            case "bounds": prefixCode = "56578b74240c8dbe1c04000083ec0c"; suffixCode = "5f5ec3"; break;
            // Replace the final engine call with a capture of its stack argument.
            case "terrainScale": prefixCode = "56578b74240c8b442410d9006a00"; suffixCode = "5889065f5ec3"; break;
            case "physicsFace": prefixCode = "56578b7c240c"; suffixCode = "58890783c4045f5ec3"; break;
            // Copy the scale passed to AddTerrain back to the fixture's source buffer.
            case "collisionScale":
                prefixCode = "56578b74240c8b7c241083ec70" +
                    "8b07894424408b4704894424448b47088944244883ec18";
                suffixCode = "8d4c242829c889470c" +
                    "8b44244089078b4424448947048b44244889470883c4705f5ec3";
                break;
            case "stampHalf":
                // The process has no retail PE data section. Load the same constant
                // from the fixture argument, then execute the original gate and add.
                body = "f30f103f" + body.Substring(16);
                prefixCode = "56578b74240c8b7c241039ff";
                suffixCode = "0f944704f30f113f5f5ec3";
                break;
            case "stampWidth":
                prefixCode = "56578b74240c8b7c241083ec548b0789442450f30f10470839ff";
                suffixCode = "0f944704f30f110783c4545f5ec3";
                break;
            case "stampAlignX":
            case "stampAlignY":
                body = "d907" + body.Substring(12);
                prefixCode = "56578b74240c8b7c241039ff";
                suffixCode = "0f944704d91f5f5ec3";
                break;
            case "zoningBounds":
                body = body.Substring(0, body.Length - 12) + "8b0e";
                prefixCode = "5356578b5c24108b74241483ec388b3e897c242c8b46048944241c39ff";
                suffixCode = "0f94460c893e8b44242c8946048b44241c89460883c4385f5e5bc3";
                break;
            case "controlExtentX":
            case "controlExtentY":
                // Supply the stock constant and GameTerrain pointer from fixture memory.
                body = "d907" + body.Substring(12, 4) + "8b4708" + body.Substring(26);
                prefixCode = "56578b74240c8b7c241039ffb878563412";
                suffixCode = "0f94470489470cd91f5f5ec3";
                break;
            default: throw new ArgumentException("unknown stub");
        }
        byte[] prefix = Hex(prefixCode);
        byte[] suffix = Hex(suffixCode);
        byte[] stub = Hex(body);
        byte[] code = new byte[prefix.Length + stub.Length + suffix.Length];
        prefix.CopyTo(code, 0);
        stub.CopyTo(code, prefix.Length);
        suffix.CopyTo(code, prefix.Length + stub.Length);
        UIntPtr length = (UIntPtr)(uint)code.Length;
        IntPtr region = VirtualAlloc(IntPtr.Zero, length, 0x3000, 4);
        IntPtr instance = Marshal.AllocHGlobal(0x27a24);
        IntPtr source = Marshal.AllocHGlobal(16);
        try {
            if (region == IntPtr.Zero) throw new InvalidOperationException("VirtualAlloc failed");
            Marshal.Copy(code, 0, region, code.Length);
            uint old;
            if (!VirtualProtect(region, length, 0x20, out old))
                throw new InvalidOperationException("VirtualProtect failed");
            Marshal.Copy(new byte[0x27a24], 0, instance, 0x27a24);
            Marshal.WriteInt32(instance, 0x3d4, unchecked((int)environment));
            Marshal.WriteInt32(instance, 0x27a20, unchecked((int)environment));
            Marshal.WriteIntPtr(instance, 0x38, instance);
            Marshal.WriteInt32(source, 0, name == "terrainScale" ? 0x44800000 : 128);
            Marshal.WriteInt32(source, 4, 128);
            if (name == "zoningBounds") {
                Marshal.WriteIntPtr(instance, 0x0c, instance);
                Marshal.WriteInt32(instance, 0x44c, 256);
                Marshal.WriteInt32(instance, 0x450, 224);
            }
            if (name == "collisionScale") {
                Marshal.WriteIntPtr(instance, 0x0c, instance);
                Marshal.WriteInt32(source, 0, 0x44800000);
                Marshal.WriteInt32(source, 4, 0x44800000);
                Marshal.WriteInt32(source, 8, 0x45000000);
            }
            if (stamp) {
                int input = name == "stampHalf" ? 0x44800000 : name == "stampWidth" ? 0x40800000 : 0x45000000;
                Marshal.WriteInt32(source, 0, input);
                Marshal.WriteInt32(source, 4, 0);
                Marshal.WriteInt32(source, 8, 0x3f800000);
            }
            if (control) {
                Marshal.WriteInt32(source, 0, 0x45000000);
                Marshal.WriteInt32(source, 4, 0);
                Marshal.WriteIntPtr(source, 8, environment == 0 ? IntPtr.Zero : instance);
                Marshal.WriteIntPtr(instance, 0x18, environment == 0xa0b60dde ? IntPtr.Zero : instance);
            }
            int[] stock = {unchecked((int)0xc4800000), unchecked((int)0xc4800000),
                unchecked((int)0xc4800000), 0x44800000, 0x44800000, 0x44800000};
            Marshal.Copy(stock, 0, IntPtr.Add(instance, 0x41c), stock.Length);
            ((Run)Marshal.GetDelegateForFunctionPointer(region, typeof(Run)))(instance, source);
            bool expanded = environment == 0x81ced36f;
            if (control) {
                int expected = expanded ? 0x45800000 : 0x45000000;
                if (Marshal.ReadInt32(source) != expected || Marshal.ReadByte(source, 4) != 1 ||
                    Marshal.ReadInt32(source, 12) != 0x12345678)
                    throw new InvalidOperationException("wrong control extent, flags, or register");
            } else if (name == "zoningBounds") {
                int expectedX = expanded ? 256 : 128;
                int expectedY = expanded ? 224 : 128;
                if (Marshal.ReadInt32(source, 0) != expectedX || Marshal.ReadInt32(source, 4) != expectedX ||
                    Marshal.ReadInt32(source, 8) != expectedY || Marshal.ReadByte(source, 12) != 1)
                    throw new InvalidOperationException("wrong zoning bounds or flags");
            } else if (stamp) {
                int expected = name == "stampHalf" ? 0x44800000 : name == "stampWidth" ? 0x3e800000 : 0x45000000;
                if (expanded) expected += 0x00800000;
                if (Marshal.ReadInt32(source) != expected || Marshal.ReadByte(source, 4) != 1)
                    throw new InvalidOperationException("wrong stamp geometry or flags");
            } else if (name == "collisionScale") {
                int expected = expanded ? 0x45000000 : 0x44800000;
                if (Marshal.ReadInt32(source) != expected || Marshal.ReadInt32(source, 4) != expected ||
                    Marshal.ReadInt32(source, 8) != 0x45000000 || Marshal.ReadInt32(source, 12) != 0)
                    throw new InvalidOperationException("wrong collision scale");
            } else if (name == "terrainScale" || name == "physicsFace") {
                int expected = name == "terrainScale" ? (expanded ? 0x45000000 : 0x44800000) :
                    (expanded ? 512 : 256);
                if (Marshal.ReadInt32(instance) != expected)
                    throw new InvalidOperationException("wrong terrain argument");
            } else if (name == "grid") {
                int expected = expanded ? 256 : 128;
                if (Marshal.ReadInt32(instance, 0x44c) != expected ||
                    Marshal.ReadInt32(instance, 0x450) != expected)
                    throw new InvalidOperationException("wrong grid");
            } else {
                if (expanded) {
                    stock[0] = stock[1] = unchecked((int)0xc5000000);
                    stock[3] = stock[4] = 0x45000000;
                }
                for (int i=0; i<stock.Length; i++)
                    if (Marshal.ReadInt32(instance, 0x41c + i*4) != stock[i])
                        throw new InvalidOperationException("wrong bounds");
                if (Marshal.ReadInt32(instance, 0x434) != stock[0])
                    throw new InvalidOperationException("displaced x87 store missing");
            }
        } finally {
            Marshal.FreeHGlobal(instance);
            Marshal.FreeHGlobal(source);
            if (region != IntPtr.Zero) VirtualFree(region, UIntPtr.Zero, 0x8000);
        }
    }
}
'@
foreach ($stub in $sizing.stubs) {
    foreach ($environment in @('81CED36F', 'A0B60DDE', '81BD4AE9', '00000000')) {
        [EdgewaterSizingFixture]::Test($stub.body, $stub.name, [Convert]::ToUInt32($environment, 16))
        Write-Output "$($stub.name) $environment passed"
    }
}
