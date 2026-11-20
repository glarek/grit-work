#!/usr/bin/env python3
"""Separate application regressions from measured OCR recognition limitations."""
import argparse
import html
import json
from pathlib import Path
import unicodedata

root=Path(__file__).resolve().parents[1]/'all-artifacts/ocr-robustness'
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--language',choices=['auto','sv-SE','en-US'],default='auto')
language=parser.parse_args().language
suffix='' if language=='auto' else '-'+language
cases=json.loads((root/'fixtures/manifest.json').read_text())
results={row['id']:row for row in json.loads((root/('windows-results'+suffix+'.json')).read_text(encoding='utf-8-sig'))}

def normalized(text):
    return ' '.join(unicodedata.normalize('NFC',text).split())

def edits(left,right):
    previous=list(range(len(right)+1))
    for i,a in enumerate(left,1):
        current=[i]
        for j,b in enumerate(right,1):
            current.append(min(current[-1]+1,previous[j]+1,previous[j-1]+(a!=b)))
        previous=current
    return previous[-1]

rows=[]
for case in cases:
    result=results[case['id']]
    output=result.get('output') or {}
    text=output.get('text','')
    missing=[item for item in case.get('required',[]) if item not in text]
    regions=output.get('regions') or []
    bounds_ok=all(r['width']>0 and r['height']>0 and 0<=r['x']<r['x']+r['width']<=case.get('width',0) and 0<=r['y']<r['y']+r['height']<=case.get('height',0) for r in regions)
    if case['kind']=='invalid':passed=result['exit_code']!=0
    elif case['kind']=='blank':passed=result['exit_code']==0 and not text.strip() and not regions
    else:passed=result['exit_code']==0 and bounds_ok and (not missing if case['kind']=='required' else True)
    if language!='auto' and case['kind']!='invalid':passed=passed and output.get('language')==language
    expected=normalized(case.get('expected',''));actual=normalized(text)
    cer=round(100*edits(expected,actual)/len(expected),2) if expected else None
    rows.append({'id':case['id'],'kind':case['kind'],'passed':passed,'exact_text':expected==actual,'character_error_percent':cer,'missing_identifiers':missing,'language':output.get('language'),'milliseconds':result['milliseconds'],'expected':case.get('expected',''),'actual':text,'file':case['file']})
report={'synthetic_only':True,'image_cases':len(rows),'application_checks_passed':sum(row['passed'] for row in rows),'recognition_exact_cases':sum(row['exact_text'] for row in rows if row['kind'] not in ['invalid','blank']),'text_image_cases':sum(row['kind'] not in ['invalid','blank'] for row in rows),'languages':sorted({row['language'] for row in rows if row['language']}),'notes':['OCR-kvalitet mäts separat från krascher, formatfel och dataförlust.','CER normaliserar Unicode till NFC och whitespace. Inga bokstäver eller siffror rättas automatiskt.','Språket kontrolleras för varje lyckad läsning. Små tecken, tekniska symboler och andra skriftsystem kan fortfarande feltolkas.','AI-leverantörernas textformat testas lokalt. Dessa tester verifierar inte modellernas bildförståelse eller fullständigt skydd mot promptinjektion.'],'cases':rows}
(root/('report'+suffix+'.json')).write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
esc=html.escape
body=''.join(f'<tr><td><a href="fixtures/{esc(row["file"],quote=True)}">{esc(row["id"])}</a></td><td>{"Godkänt" if row["passed"] else "FEL"}</td><td>{str(row["character_error_percent"])+" %" if row["character_error_percent"] is not None else "—"}</td><td>{esc(", ".join(row["missing_identifiers"]) or "—")}</td><td>{row["milliseconds"]} ms</td></tr><tr><td colspan="5"><details><summary>Förväntad och läst text</summary><div class="comparison"><pre>{esc(row["expected"])}</pre><pre>{esc(row["actual"])}</pre></div></details></td></tr>' for row in rows)
page=f'''<!doctype html><html lang="sv"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>GRIT Work – OCR-robusthet</title><style>body{{margin:40px auto;max-width:1150px;padding:0 24px;font:15px/1.6 system-ui;background:#141217;color:#f5f1f8}}h1{{font-size:32px}}p,li,summary{{color:#bdb4c6}}a{{color:#d7a7ef}}table{{width:100%;border-collapse:collapse;font-size:13px}}th,td{{text-align:left;padding:10px;border-bottom:1px solid #322b38}}th{{color:#d7a7ef}}pre{{white-space:pre-wrap;overflow-wrap:anywhere;unicode-bidi:plaintext;background:#221d28;padding:16px;border-radius:8px;min-width:0}}.comparison{{display:grid;grid-template-columns:1fr 1fr;gap:16px}}.metric{{font-size:26px;font-weight:650}}@media(max-width:700px){{.comparison{{grid-template-columns:1fr}}table{{font-size:11px}}td,th{{padding:6px}}}}</style><h1>OCR: tecken, underlag och verklig läsning</h1><p>Windows Media OCR via appens produktionskod. Syntetiska bilder, samma källkod som appen.</p><p class="metric">{report['application_checks_passed']}/{len(rows)} bildfall klarar funktionskraven</p><p>Exakt normaliserad text i {report['recognition_exact_cases']}/{report['text_image_cases']} textbilder. Detta är en liten variationssvit, inte ett generellt noggrannhetsmått.</p><ul>{''.join('<li>'+esc(note)+'</li>' for note in report['notes'])}</ul><h2>Åtgärdade regressioner</h2><ul><li>ÅÄÖ, kombinerade accenter, symboler, enstaka tecken och skiftlägeskänsliga ID:n bevaras i äldre textrensning.</li><li>Transparenta bilder blandas mot vitt före gråskalekonvertering.</li><li>Överlappande bilddelar bevarar ord vid gränser; dubbla läsningar av samma position slås ihop.</li><li>Osynliga tecken ger inte falsk innehållsstatus. Ogiltiga koordinater avvisas.</li><li>UTF-8-BOM accepteras i AI-svar. CSV-formler bakom osynliga prefix skyddas.</li></ul><h2>Bildtesterna</h2><table><thead><tr><th>Bild</th><th>Funktionskrav</th><th>Teckenfel</th><th>Saknade test-ID:n</th><th>Tid</th></tr></thead><tbody>{body}</tbody></table></html>'''
(root/('report'+suffix+'.html')).write_text(page)
print(json.dumps({key:value for key,value in report.items() if key not in ['cases','notes']},ensure_ascii=False))
raise SystemExit(0 if all(row['passed'] for row in rows) else 1)
