param([int]$GamePid,[string]$Feed,[string]$Requests,[int]$DesktopPid=0)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Windows.Forms,System.Drawing
Add-Type -ReferencedAssemblies System.Windows.Forms,System.Drawing @"
using System;
using System.Runtime.InteropServices;
using System.Windows.Forms;
public class DevtoolOverlay : Form {
 public bool Picking;
 public static void WatchParent(int pid) {
   var thread=new System.Threading.Thread(()=> {try {var parent=System.Diagnostics.Process.GetProcessById(pid);parent.WaitForExit();}catch {} Environment.Exit(0);});
   thread.IsBackground=true;thread.Start();
 }
 protected override bool ShowWithoutActivation { get { return !Picking; } }
 protected override CreateParams CreateParams { get { var p=base.CreateParams; p.ExStyle|=0x80; if(!Picking)p.ExStyle|=0x20|0x80000; return p; } }
 public void Mode(bool picking) { Picking=picking; RecreateHandle(); }
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int Left,Top,Right,Bottom;}
 [StructLayout(LayoutKind.Sequential)] public struct Point {public int X,Y;}
 [DllImport("user32")] public static extern bool GetClientRect(IntPtr h,out Rect r);
 [DllImport("user32")] public static extern bool ClientToScreen(IntPtr h,ref Point p);
 [DllImport("user32")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr p);
}
"@
if($DesktopPid -gt 0){[DevtoolOverlay]::WatchParent($DesktopPid)}
[void][DevtoolOverlay]::SetThreadDpiAwarenessContext([IntPtr](-4))
$game=Get-Process -Id $GamePid
$started=([DateTimeOffset]$game.StartTime).ToUnixTimeMilliseconds()
$form=[DevtoolOverlay]::new()
$form.Text='OpenSCP Devtool Overlay';$form.FormBorderStyle='None';$form.StartPosition='Manual';$form.ShowInTaskbar=$false;$form.TopMost=$true
$form.BackColor=[Drawing.Color]::MidnightBlue;$form.Opacity=0.25;$form.KeyPreview=$true
$script:frame=$null;$script:box=$null
function Send($kind,$node=$null,$message=$null){[Console]::WriteLine((@{message=$message;hwnd=$form.Handle.ToInt64();kind=$kind;pid=$GamePid;node=$node;sequence=$script:frame.sequence;time=$script:frame.time}|ConvertTo-Json -Compress));[Console]::Out.Flush()}
function Read-Frame {
 try {
  $s=[IO.File]::Open($Feed,'Open','Read',([IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete))
  $r=[IO.StreamReader]::new($s)
  try {$next=$r.ReadToEnd()|ConvertFrom-Json} finally {$r.Dispose()}
  if($next.pid -ne $GamePid -or $next.time -lt $started){return}
  if($script:frame -and $next.time -eq $script:frame.time -and $next.sequence -eq $script:frame.sequence){return}
  $script:frame=$next;$script:box=$null;$form.Hide();Send 'snapshot'
 } catch { Send 'read-error' }
}
function Show-Overlay($pick) {
 $game.Refresh()
 if($game.HasExited -or !$script:frame){return}
 $r=[DevtoolOverlay+Rect]::new();$p=[DevtoolOverlay+Point]::new()
 if(![DevtoolOverlay]::GetClientRect($game.MainWindowHandle,[ref]$r) -or ![DevtoolOverlay]::ClientToScreen($game.MainWindowHandle,[ref]$p)){return}
 $form.Hide();$form.Mode($pick)
 $form.Bounds=[Drawing.Rectangle]::new($p.X,$p.Y,$r.Right,$r.Bottom)
 $form.Show();if($pick){$form.Activate()};$form.Invalidate();Send $(if($pick){'picking'}else{'highlighted'})
}
$form.Add_Paint({param($s,$e)
 if($script:box -and $script:frame){
  $r=$script:box.rect;$sx=$form.Width/$script:frame.viewport[0];$sy=$form.Height/$script:frame.viewport[1]
  $pen=[Drawing.Pen]::new([Drawing.Color]::Cyan,3)
  try{$e.Graphics.DrawRectangle($pen,[single]($r[0]*$sx),[single]($r[1]*$sy),[single](($r[2]-$r[0])*$sx),[single](($r[3]-$r[1])*$sy))}finally{$pen.Dispose()}
 }
})
$form.Add_MouseMove({param($s,$e)
 if(!$form.Picking -or !$script:frame){return}
 $x=$e.X*$script:frame.viewport[0]/$form.Width;$y=$e.Y*$script:frame.viewport[1]/$form.Height
 $best=$null;$area=[double]::PositiveInfinity
 foreach($n in $script:frame.boxes){$r=$n.rect;if($r -and $x -ge $r[0] -and $x -lt $r[2] -and $y -ge $r[1] -and $y -lt $r[3]){$a=($r[2]-$r[0])*($r[3]-$r[1]);if($a -le $area){$area=$a;$best=$n}}}
 if($script:box -ne $best){$script:box=$best;$form.Invalidate()}
})
$form.Add_MouseDown({param($s,$e) if($form.Picking -and $e.Button -eq 'Left' -and $script:box){Send 'selected' $script:box.i;$form.Hide()}})
$form.Add_KeyDown({param($s,$e) if($e.KeyCode -eq 'Escape'){$form.Hide();Send 'cancelled';$e.SuppressKeyPress=$true}})
$null=$form.Handle
$watch=[IO.FileSystemWatcher]::new([IO.Path]::GetDirectoryName($Feed),[IO.Path]::GetFileName($Feed));$watch.SynchronizingObject=$form
$changed={Read-Frame};$watch.add_Changed($changed);$watch.add_Created($changed);$watch.add_Renamed($changed);$watch.EnableRaisingEvents=$true
$commands=[IO.FileSystemWatcher]::new($Requests,'*.json');$commands.SynchronizingObject=$form
$onCommand={param($s,$e)
 try {
  $c=[IO.File]::ReadAllText($e.FullPath)|ConvertFrom-Json
  if($c.action -eq 'close'){$form.Close();[Windows.Forms.Application]::ExitThread();return}
  if($c.action -eq 'clear'){$form.Hide();Send 'cleared';return}
  if(!$script:frame -or $c.sequence -ne $script:frame.sequence -or $c.time -ne $script:frame.time){Send 'stale';return}
  if($c.action -eq 'pick'){$script:box=$null;Show-Overlay $true}
  elseif($c.action -eq 'highlight'){$script:box=$script:frame.boxes|Where-Object {$_.i -eq $c.node}|Select-Object -First 1;if($script:box){Show-Overlay $false}else{$form.Hide()}}
 }catch{Send 'command-error' $null $_.Exception.Message}finally{Remove-Item -LiteralPath $e.FullPath -ErrorAction SilentlyContinue}
};$commands.add_Created($onCommand);$commands.add_Renamed($onCommand);$commands.EnableRaisingEvents=$true
$game.EnableRaisingEvents=$true
# Process exit closes this helper without polling the game.
$exit=Register-ObjectEvent $game Exited -Action {[Environment]::Exit(0)}
Read-Frame;Send 'ready'
try{[Windows.Forms.Application]::Run()}finally{$watch.Dispose();$commands.Dispose();$form.Dispose();Unregister-Event -SourceIdentifier $exit.Name -ErrorAction SilentlyContinue}
