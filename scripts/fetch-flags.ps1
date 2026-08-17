# Downloads the country flags the server list draws.
#
# Windows has no flag glyphs. Its emoji font deliberately renders a regional
# indicator pair as the two letters it is built from, so 🇩🇪 comes out as "DE" —
# not a missing-font box, which is why it looks like a deliberate choice rather
# than a gap. No system font on Windows can draw these, so the flags have to be
# pictures.
#
# 40px wide PNGs from flagcdn, which serves the ISO 3166-1 alpha-2 set. About a
# kilobyte each, so the whole set is smaller than one of the fonts.
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
New-Item -ItemType Directory -Force -Path resources\flags | Out-Null

# Every alpha-2 code flagcdn publishes. Kept as a literal list rather than
# scraped, so a build is not at the mercy of an index page changing shape.
$codes = @(
  'ad','ae','af','ag','ai','al','am','ao','aq','ar','as','at','au','aw','ax','az',
  'ba','bb','bd','be','bf','bg','bh','bi','bj','bl','bm','bn','bo','bq','br','bs',
  'bt','bv','bw','by','bz','ca','cc','cd','cf','cg','ch','ci','ck','cl','cm','cn',
  'co','cr','cu','cv','cw','cx','cy','cz','de','dj','dk','dm','do','dz','ec','ee',
  'eg','eh','er','es','et','fi','fj','fk','fm','fo','fr','ga','gb','gd','ge','gf',
  'gg','gh','gi','gl','gm','gn','gp','gq','gr','gs','gt','gu','gw','gy','hk','hm',
  'hn','hr','ht','hu','id','ie','il','im','in','io','iq','ir','is','it','je','jm',
  'jo','jp','ke','kg','kh','ki','km','kn','kp','kr','kw','ky','kz','la','lb','lc',
  'li','lk','lr','ls','lt','lu','lv','ly','ma','mc','md','me','mf','mg','mh','mk',
  'ml','mm','mn','mo','mp','mq','mr','ms','mt','mu','mv','mw','mx','my','mz','na',
  'nc','ne','nf','ng','ni','nl','no','np','nr','nu','nz','om','pa','pe','pf','pg',
  'ph','pk','pl','pm','pn','pr','ps','pt','pw','py','qa','re','ro','rs','ru','rw',
  'sa','sb','sc','sd','se','sg','sh','si','sj','sk','sl','sm','sn','so','sr','ss',
  'st','sv','sx','sy','sz','tc','td','tf','tg','th','tj','tk','tl','tm','tn','to',
  'tr','tt','tv','tw','tz','ua','ug','um','us','uy','uz','va','vc','ve','vg','vi',
  'vn','vu','wf','ws','ye','yt','za','zm','zw'
)

$have = 0
$got = 0
foreach ($code in $codes) {
    $out = "resources\flags\$code.png"
    if (Test-Path -LiteralPath $out) { $have++; continue }
    try {
        Invoke-WebRequest -Uri "https://flagcdn.com/w40/$code.png" -OutFile $out -UseBasicParsing
        $got++
    } catch {
        # A code the CDN does not carry is not worth failing a build over: the
        # app falls back to a globe for anything it has no picture of.
        Write-Host "  no flag for $code"
        Remove-Item -LiteralPath $out -Force -ErrorAction SilentlyContinue
    }
}
Write-Output "flags: $got downloaded, $have already present, $((Get-ChildItem resources\flags -Filter *.png).Count) total"
