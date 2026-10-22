# Launch a Roblox account via Volt's stored credentials.
# ENV: VOLT_CRED_TARGET = credential manager target name
#      VOLT_PLACE_ID    = place to join
# Outputs JSON: { "ok": true, "pid": 1234 } or { "error": "..." }
param()
$ErrorActionPreference = 'Stop'
try {
    Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Text;

public class CredMan {
    [DllImport("advapi32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
    static extern bool CredReadW(string target, int type, int flags, out IntPtr cred);
    [DllImport("advapi32.dll")] static extern void CredFree(IntPtr cred);

    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
    struct CREDENTIAL {
        public int Flags; public int Type;
        public string TargetName; public string Comment;
        public long LastWritten;
        public int CredentialBlobSize;
        public IntPtr CredentialBlob;
        public int Persist; public int AttributeCount;
        public IntPtr Attributes; public string TargetAlias;
        public string UserName;
    }

    public static string Read(string target) {
        IntPtr ptr;
        if (!CredReadW(target, 1, 0, out ptr))
            throw new Exception("CredRead failed: " + Marshal.GetLastWin32Error());
        try {
            var c = (CREDENTIAL)Marshal.PtrToStructure(ptr, typeof(CREDENTIAL));
            if (c.CredentialBlobSize <= 0) throw new Exception("empty credential");
            byte[] bytes = new byte[c.CredentialBlobSize];
            Marshal.Copy(c.CredentialBlob, bytes, 0, bytes.Length);
            return Encoding.UTF8.GetString(bytes);
        } finally { CredFree(ptr); }
    }
}
"@ -Language CSharp -ErrorAction SilentlyContinue

    $target = $env:VOLT_CRED_TARGET
    $placeId = $env:VOLT_PLACE_ID

    if (-not $target -or -not $placeId) {
        Write-Output '{"error":"VOLT_CRED_TARGET and VOLT_PLACE_ID required"}'
        exit 1
    }

    # read cookie from credential manager
    $cookie = [CredMan]::Read($target)
    if (-not $cookie -or $cookie.Length -lt 10) {
        Write-Output '{"error":"cookie is empty or too short"}'
        exit 1
    }

    # get CSRF token
    $csrfResp = try {
        Invoke-WebRequest -Uri "https://auth.roblox.com/v2/logout" -Method POST `
            -Headers @{ "Cookie" = ".ROBLOSECURITY=$cookie" } `
            -UseBasicParsing -ErrorAction Stop
    } catch {
        $_.Exception.Response
    }
    $csrf = ""
    if ($csrfResp -and $csrfResp.Headers) {
        if ($csrfResp.Headers["x-csrf-token"]) {
            $csrf = $csrfResp.Headers["x-csrf-token"]
        } elseif ($csrfResp.Headers.ContainsKey("x-csrf-token")) {
            $csrf = $csrfResp.Headers["x-csrf-token"]
        }
    }
    # retry via exception response headers
    if (-not $csrf) {
        try {
            Invoke-RestMethod -Uri "https://auth.roblox.com/v2/logout" -Method POST `
                -Headers @{ "Cookie" = ".ROBLOSECURITY=$cookie" } -ErrorAction Stop
        } catch {
            $resp = $_.Exception.Response
            if ($resp.Headers -and $resp.Headers["x-csrf-token"]) {
                $csrf = $resp.Headers["x-csrf-token"] | Select-Object -First 1
            }
        }
    }

    if (-not $csrf) {
        Write-Output '{"error":"failed to get CSRF token - cookie may be dead"}'
        exit 1
    }

    # get auth ticket
    $ticketResp = Invoke-WebRequest -Uri "https://auth.roblox.com/v1/authentication-ticket" -Method POST `
        -Headers @{
            "Cookie" = ".ROBLOSECURITY=$cookie"
            "x-csrf-token" = $csrf
            "Referer" = "https://www.roblox.com"
        } `
        -UseBasicParsing -ErrorAction Stop

    $ticket = $ticketResp.Headers["rbx-authentication-ticket"] | Select-Object -First 1
    if (-not $ticket) {
        Write-Output '{"error":"no auth ticket in response"}'
        exit 1
    }

    # build launch URL
    $launcherUrl = "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestGame&placeId=$placeId"
    $encoded = [uri]::EscapeDataString($launcherUrl)
    $launchUri = "roblox-player:1+launchmode:play+gameinfo:$ticket+launchtime:$([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())+placelauncherurl:$encoded+browsertrackerid:0+robloxLocale:en_us+gameLocale:en_us"

    $proc = Start-Process -FilePath $launchUri -PassThru -ErrorAction Stop
    # roblox-player: protocol hands off to RobloxPlayerBeta.exe
    # wait a moment for the real process to spawn
    Start-Sleep -Seconds 2
    $rbx = Get-Process RobloxPlayerBeta -ErrorAction SilentlyContinue | Sort-Object StartTime -Descending | Select-Object -First 1
    $pid_out = if ($rbx) { $rbx.Id } else { 0 }

    Write-Output "{`"ok`":true,`"pid`":$pid_out}"
} catch {
    $msg = $_.Exception.Message -replace '"','\"'
    Write-Output "{`"error`":`"$msg`"}"
    exit 1
}
