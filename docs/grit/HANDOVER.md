# GRIT Work — handover

Senast uppdaterad 2026-11-20. **1.1.4-rc.4 är publicerad** som osignerad Windows x64-förhandsversion: [release](https://github.com/glarek/grit-work/releases/tag/grit-v1.1.4-rc.4). Källrevision `6b8763f`, Windows run `44761980199`, installerare och portabel zip samt uppgradering från rc.3 är verifierade. Dashboarden har ljust/mörkt tema och Spiris-användarval; paketeringen säkerställer svensk och engelsk OCR. Windows OCR:s kvarstående I/l-fel är dokumenterade. Se [VALIDATION](VALIDATION.md) för kontroller, checksummor och begränsningar.

## Arbetsmiljö

- Repo: `glarek/grit-work`, gren `grit/initial-app`, utkast till PR #1. Använd `gh --repo glarek/grit-work` eftersom originalets upstream också finns som remote.
- Lokal arbetskatalog: `/srv/gritprojects/GitHub/grit-work` (fysisk `/home/adrian/Projects/GitHub/grit-work`).
- Orkestrering sker i den redan körande Herdr-instansen, samma space. Läs `AGENTS.md`, skillen `herdr-orchestration` och `herdr --skill` före delegering. Senaste space: `wM`, root `wM:p1`. Kontrollera live-status innan gamla agentnamn återanvänds. Inga agenter har aktivt filägarskap efter denna leverans.
- Frontend använder Svelte 4. Bits UI 0.21/Melt 0.76 ger gemensamma shadcn-komponenter. En oplanerad uppgradering till Svelte 5 är inte del av uppgiften. Dialog/Select använder Melt direkt för att behålla reaktiva slots i Svelte 4.

## Viktiga delar

- `src/routes/grit/GritWork.svelte`: kompakt dagslista, utkast, avrundning, godkännande och jobbstatus.
- `ReportRowEditor.svelte`: lokal redigeringskopia; Avbryt ändrar inte en befintlig rad. `ReportingDialog.svelte` och `reportingContext.ts`: färska rapporteringsval, tidigare rader och slutgodkännande.
- `src/routes/settings/Settings.svelte`, `GritSettings.svelte`, `CaptureSettings.svelte`: gemensamma inställningar. `Onboarding.svelte`: första startens uppgifter/samtycke eller uttryckligt Avvakta.
- `src/lib/components/ui/`: gemensamma kontroller. Switch har 44 × 24 px spår och 20 px prick med 2 px marginal i båda lägena. `LocalizedDatePicker.svelte`: svenska ISO-datum; `weekStartsOn=0` är måndag eftersom denna biblioteksversions värde är relativt språkets första veckodag.
- `image_schedule.rs`: samma nästa starttid används av bakgrundsjobbet och vänstermenyn. Ändringar väcker schemat via Notify. Jobbet behandlar högst 32 utvalda bilder sekventiellt inom befintlig tidsgräns. Off och saknat samtycke uppdaterar endast JSON.
- `ImageInterpretationSettings.svelte`: Off/OCR/AI, eget intervall, sparkrav före körning och status. `image_interpretation.rs`: gemensam JSON-cache och validerat rapportunderlag. `image_ocr.rs`: begränsad Windows OCR med svensk språkpreferens och balanserad WinRT-initiering.
- `src-tauri/src/grit.rs`: Spiris, projektregister, utkast, arkiv, behörighetskontroller och överföringskvitton. `grit_cli.rs`: CLI-anrop och validerat AI-svar. `monitor.rs`/`screenshot.rs`: Windows integritetskontroller före/efter minnesbaserad fångst.

## Levererad rc.3

- `get_day_evidence` och `clip_day_evidence` bevarar original-ID men klipper intervall vid lokal midnatt. `grit_read::Archive` kan betyda blandat lokalt/verifierat arkiv. Endast mål- och nästa dygn inspekteras; utgångna dygn och trasiga arkiv respekteras. Rå export/rensning är oförändrad.
- Utkast tillåter idag. Manuell generering använder färdiga bildtolkningar (`allow_incomplete=true`) och tydliga tomma statusfält för resterande bilder. Automatisk förberedelse av avslutade dagar är strikt. `Report.basis` beskriver ögonblicksbilden och bevaras vid manuell redigering. Ingen tid får skapas från bildfrekvens. Spiris-överföring är fortfarande för avslutade dygn.
- Standardbildbredd 0 = original. `resolution_policy` migrerar gamla 1280/1440-gränser bara en gång; nya uttryckliga gränser behålls. AI delar i 1600-pixelbitar med 80-pixelöverlapp; ingen thumbnail. OCR:s textmängd är inte ett kvalitets-/korrekthetsmått.
- Arkiv är valfritt; utan arkiv ingen automatisk lokal rensning. Äldre exakt fel om saknat arkiv döljs, andra insamlingsfel behålls.
- Spiris/AI-badges bygger enbart på sparade testresultat och lokal pollning. Inställningsbyte gör test ogiltigt, 30 minuter gör test gammalt. AI-rapport och eventuell AI-bildmodell har separata resultat.
- Agentarbetet finns synligt i Herdr wM. Tillfällig överlämning/loggar: `/tmp/grit-refinement/HANDOVER.md`. Native byggresultat och checksummor är sparade i VALIDATION.

## Kontrakt som måste bevaras

- Bildtolkning är av som standard och skriver tomt bildinnehåll. Alla lägen skriver `image-interpretations/YYYY-MM-DD.json`. Rapporter får enbart loggar och sparad tolkning, aldrig råa skärmbilder. Äldre automatisk OCR vid bildtagning är avstängd. Fel kräver uttryckligt omförsök.
- AI-samtycke är av som standard. Spiris-hemligheter ligger i OS-nyckellagret, aldrig i repo, loggar, fixtures eller agentprompter.
- Bilder/loggar får rensas lokalt först efter komplett arkiv och verifierad återläsning/checksumma. Ofärdiga rapporters underlag behålls.
- Tidigare app-/domän-/titelundantag rensas vid konfigurationsnormalisering, enligt användarens uttryckliga val. Ingen integritetspanel visas. Befintliga fångstkontroller finns kvar i koden. Alla skärmar/fönster kontrolleras före och efter Windows-fångst. Okänd webbadress med aktiva domänundantag stoppar bilden. Ingen spärrad bild skrivs till disk.
- Tid avrundas uppåt till halvtimmar per projekt/dygn. Råtid bevaras; manuell tid kräver dagbokstext men ingen separat orsaksruta. Historiska texter är endast skrivstilsstöd.
- `Row.time_code_id` har standard 0 för äldre data. Generering sätter aktivitet/tidskod/artikel till 0; användaren väljer vid granskningen. Backend validerar aktiva registerval, projektstatus och medlemskap precis före varje POST.
- Spiris API exponerar inga projektspecifika begränsningslistor för aktivitet/tidskod/artikel. UI får inte hitta på sådana eller hävda att ett GET-test bevisar skrivbehörighet.
- Samma dags befintliga tid blockerar överföring. Tidigare 90 dagars rader visas endast som stöd. Osäkra/delvisa skrivningar låses; aldrig automatiska POST-omförsök. Riktiga tidsregistreringar kräver uttryckligt godkännande i appen.

## Verifiera och leverera

`npm ci` med Node 22/npm 10; `npm run verify:frontend`; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace --all-targets -- --test-threads=1`. Kör inte det ignorerade testet mot verklig skärm som rutin.

Webbläsartester körs **sekventiellt**: `node scripts/with-grit-preview.mjs scripts/test-grit-ui.mjs`, därefter `scripts/test-grit-workflow.mjs` via samma wrapper. Wrappern äger Vite på port 5183. Fixtures är syntetiska; inga API-anrop görs.

`node scripts/with-grit-preview.mjs scripts/export-grit-review.mjs` skapar 14 PDF-sidor och urklipp under `all-artifacts/interface-review/`. Typsnitten är lokalt paketerade.

Push på grenen startar `.github/workflows/windows-review.yml`: native tester/Clippy och optimerad NSIS. Ny push avbryter tidigare körning. Hämta artefakten `Grit-Work-Windows-review`, verifiera SHA-256 och använd skillen `windows-vm-ssh` för testinstallation på `grit-windows`. Bevara aktiva användarprocesser och data; starta inte verklig insamling som del av installationskontrollen. Spara slutliga run-ID, checksumma och VM-resultat i VALIDATION. Dokumentationscommit efter bygget använder `[skip ci]`.

Aktuell leverans: `all-artifacts/windows-rc4/release-assets/`, byggd från `6b8763f` i run `44761980199`. GitHub-taggen är `grit-v1.1.4-rc.4`; den pekar exakt på den byggda källrevisionen. Releasens tre filer är installerare, portabel zip och SHA256SUMS.txt. Full provenance, GitHub-återläsning och VM-resultat finns under `all-artifacts/windows-rc4/` och i VALIDATION. Installerad rc.4 i VM:en behålls; appen startades inte. `windows-refinement/` innehåller föregående rc.3. Nya förhandsreleaser använder `grit-v`-taggar och det verifierade Windows-jobbets artefakter; originalets ärvda flersystemsrelease används inte för Windows-forken.

Arbetsloggar: `/tmp/grit-refinement/`. Alla agenter är frysta; inga aktiva filägarskap eller nödvändiga uppgifter återstår. Testpanelen `wM:p7` stängdes efter att endast bash återstod. Agentpaneler och worktrees bevarades. Sex ägda stagingfiler och deras tomma katalog togs bort i Windows; appen behölls och startades inte. Det ärvda macOS-jobbet har tidigare haft arkivtester som avvisar `/var`-symlänken; macOS stöds inte och Windows-leveransen ska inte beskrivas som verifierad för alla plattformar. Installationen är osignerad. Riktig Spiris-rapportering och insamling på användarens skärmar gjordes inte i testerna.
