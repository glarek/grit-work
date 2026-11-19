# GRIT Work — handover

Senast uppdaterad 2026-11-19. Implementation: `e188943`. Windows x64 är första produktens enda målplattform. Se [README](../../README.md) för användarflödet och [VALIDATION](VALIDATION.md) för verifiering och kvarvarande begränsningar.

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
- `src-tauri/src/grit.rs`: Spiris, projektregister, utkast, arkiv, behörighetskontroller och överföringskvitton. `grit_cli.rs`: CLI-anrop och validerat AI-svar. `monitor.rs`/`screenshot.rs`: Windows integritetskontroller före/efter minnesbaserad fångst.

## Kontrakt som måste bevaras

- AI-samtycke är av som standard. Spiris-hemligheter ligger i OS-nyckellagret, aldrig i repo, loggar, fixtures eller agentprompter.
- Bilder/loggar får rensas lokalt först efter komplett arkiv och verifierad återläsning/checksumma. Ofärdiga rapporters underlag behålls.
- Alla skärmar/fönster kontrolleras före och efter Windows-fångst. Okänd webbadress med aktiva domänundantag stoppar bilden. Ingen spärrad bild skrivs till disk.
- Tid avrundas uppåt till halvtimmar per projekt/dygn. Råtid bevaras; luckor kräver manuell förklaring. Historiska texter är endast skrivstilsstöd.
- `Row.time_code_id` har standard 0 för äldre data. Generering sätter aktivitet/tidskod/artikel till 0; användaren väljer vid granskningen. Backend validerar aktiva registerval, projektstatus och medlemskap precis före varje POST.
- Spiris API exponerar inga projektspecifika begränsningslistor för aktivitet/tidskod/artikel. UI får inte hitta på sådana eller hävda att ett GET-test bevisar skrivbehörighet.
- Samma dags befintliga tid blockerar överföring. Tidigare 90 dagars rader visas endast som stöd. Osäkra/delvisa skrivningar låses; aldrig automatiska POST-omförsök. Riktiga tidsregistreringar kräver uttryckligt godkännande i appen.

## Verifiera och leverera

`npm ci` med Node 22/npm 10; `npm run verify:frontend`; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace --all-targets -- --test-threads=1`. Kör inte det ignorerade testet mot verklig skärm som rutin.

Webbläsartester körs **sekventiellt**: `node scripts/with-grit-preview.mjs scripts/test-grit-ui.mjs`, därefter `scripts/test-grit-workflow.mjs` via samma wrapper. Wrappern äger Vite på port 5183. Fixtures är syntetiska; inga API-anrop görs.

`node scripts/with-grit-preview.mjs scripts/export-grit-review.mjs` skapar tio PDF-sidor och urklipp under `all-artifacts/interface-review/`. Typsnitten är lokalt paketerade.

Push på grenen startar `.github/workflows/windows-review.yml`: native tester/Clippy och optimerad NSIS. Ny push avbryter tidigare körning. Hämta artefakten `Grit-Work-Windows-review`, verifiera SHA-256 och använd skillen `windows-vm-ssh` för testinstallation på `grit-windows`. Bevara aktiva användarprocesser och data; starta inte verklig insamling som del av installationskontrollen. Spara slutliga run-ID, checksumma och VM-resultat i VALIDATION. Dokumentationscommit efter bygget använder `[skip ci]`.

Aktuell Windows-verifiering: run `44490531400` är godkänd, liksom uppgradering och autostart i VM:n. Checksummor finns i VALIDATION. Installationsfilen finns i `all-artifacts/windows-polish/`; PDF:en finns i `all-artifacts/interface-review/`. Den temporära arbetsloggen finns i `/tmp/grit-work-redesign/HANDOVER.md`.
