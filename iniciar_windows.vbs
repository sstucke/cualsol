Set fso = CreateObject("Scripting.FileSystemObject")
Set sh = CreateObject("WScript.Shell")
root = fso.GetParentFolderName(WScript.ScriptFullName)
sh.CurrentDirectory = root
exe = root & "\target\debug\cualsol.exe"
If fso.FileExists(exe) Then
  sh.Run """" & exe & """", 1, False
Else
  sh.Run "cmd /c """ & root & "\iniciar_windows.cmd""", 1, False
End If
