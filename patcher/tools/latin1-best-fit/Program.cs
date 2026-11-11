using System.Text;

var patcher = args.Length > 0 ? args[0] : Path.Combine("..", "..");
var pairs = new List<(int Unit, byte Byte)>();
for (var unit = 0x100; unit <= 0xFFFF; unit++)
{
    var bytes = Encoding.Latin1.GetBytes(((char)unit).ToString());
    if (bytes.Length != 1)
    {
        throw new InvalidOperationException($"U+{unit:X4} gave {bytes.Length} bytes");
    }

    if (bytes[0] != (byte)'?')
    {
        pairs.Add((unit, bytes[0]));
    }
}

var table = new StringBuilder();
table.AppendLine("pub const BEST_FIT: [(u16, u8); " + pairs.Count + "] = [");
foreach (var (unit, b) in pairs)
{
    table.AppendLine($"    (0x{unit:X4}, 0x{b:X2}),");
}

table.AppendLine("];");
File.WriteAllText(Path.Combine(patcher, "src", "latin1_best_fit.rs"), table.ToString().Replace("\r\n", "\n"));
var fixture = string.Join("\n", pairs.Select(p => $"{p.Unit:X4} {p.Byte:X2}")) + "\n";
File.WriteAllText(Path.Combine(patcher, "tests", "data", "latin1-best-fit.txt"), fixture);
Console.WriteLine($"{Environment.Version}: {pairs.Count} best-fit units");
