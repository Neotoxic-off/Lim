# The info plugin on Windows: reads the lines on stdin and prints one line of facts about them.
param([string]$op)
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$text = [IO.StreamReader]::new([Console]::OpenStandardInput()).ReadToEnd() -replace "`r", ''
$lines = @($text.TrimEnd("`n") -split "`n")
switch ($op) {
    'wc' {
        $words = @($text -split '\s+' | Where-Object { $_ }).Count
        $chars = ($lines | ForEach-Object { $_.Length + 1 } | Measure-Object -Sum).Sum
        '{0} lines, {1} words, {2} chars' -f $lines.Count, $words, $chars
    }
    'sha' {
        $hash = [Security.Cryptography.SHA256]::Create().ComputeHash([Text.Encoding]::UTF8.GetBytes($text))
        ([BitConverter]::ToString($hash) -replace '-', '').ToLowerInvariant()
    }
    # DataTable.Compute only knows arithmetic, so the selection is never run as code.
    'calc' { [Data.DataTable]::new().Compute(($lines -join ' '), '') }
    default { [Console]::Error.WriteLine("unknown operation: $op"); exit 2 }
}
