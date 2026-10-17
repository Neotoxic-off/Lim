# The text plugin on Windows: the lines come on stdin, the result goes to stdout, UTF-8 both ways.
param([string]$op)
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$text = [IO.StreamReader]::new([Console]::OpenStandardInput()).ReadToEnd() -replace "`r", ''
$lines = @($text.TrimEnd("`n") -split "`n")
switch ($op) {
    'sort' { $lines | Sort-Object -CaseSensitive }
    'uniq' { $lines | Get-Unique }
    'reverse' { [array]::Reverse($lines); $lines }
    'upper' { $lines | ForEach-Object { $_.ToUpperInvariant() } }
    'lower' { $lines | ForEach-Object { $_.ToLowerInvariant() } }
    'trim' { $lines | ForEach-Object { $_.TrimEnd() } }
    'b64' { [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($text)) }
    # The decoded text keeps its own final newline.
    'b64d' { [Console]::Out.Write([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($lines -join ''))) }
    default { [Console]::Error.WriteLine("unknown operation: $op"); exit 2 }
}
