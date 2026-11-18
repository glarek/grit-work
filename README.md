# Grit Work

![Grit Works rapportvy med syntetiskt testunderlag](docs/grit/report.png)

Grit Work är en fork av [Work Review](https://github.com/wm94i/Work-Review) för granskbara tidrapporter i **Spiris Tid**. Appen behåller originalets aktivitetsinsamling och lägger till lokal femminutersinsamling, verifierat dygnsarkiv och rapportutkast via inloggade AI-CLI:er.

## Kom igång

1. Starta appen och öppna **Grit Work → Inställningar**.
2. Ange Spiris app-ID och apphemlighet, eller en befintlig åtkomsttoken. Uppgifterna sparas i operativsystemets nyckellager: Secret Service på Linux, Credential Manager på Windows, Keychain på macOS.
3. Klicka **Synka Spiris**. Välj användare, aktivitet, tidsartikel, tidskod och om tiden ska vara debiterbar.
4. Välj en skrivbar mapp för långtidslagring. En monterad nätverksdisk fungerar som en vanlig mapp. Appen erbjuder ännu inte direktanslutning till S3 eller annan objektlagring. En molnsynkad lokal mapp bevisar endast att filerna finns i den mappen, inte att molnleverantören har laddat upp dem; använd en faktiskt monterad fjärrlagring om lokal rensning ska bero på fjärrskrivningen.
5. Installera och logga in i vald CLI separat. Förvalda, bildtestade modeller är Codex `gpt-6.2-sol`, Grok `grok-4.7-build-fast` och agy `gemini-4-hydrogen-medium`. Modellnamnet kan ändras. CLI-abonnemangets tillgänglighet och kvot gäller; Grit behöver ingen AI-API-nyckel.
6. Nästa dag visar Grit frågan om gårdagens rapport. Starta genereringen, granska varje rad och välj rätt projekt. Spara utkastet och bekräfta sedan överföringen till Spiris.

**Inspelning måste vara på och datorn vaken.** Paus, låst/ledig dator, integritetsfilter och saknade skärmbehörigheter kan ge luckor. En skärmbild sparas högst en gång per fem minuter medan insamlingen är aktiv; aktivitetsloggar registrerar även programbyten. Fel i skärminsamlingen pausar inspelningen och visas i appen.

## Underlag och arkiv

Lokal data ligger alltid i operativsystemets appdata: `%LOCALAPPDATA%\grit-work` på Windows, `$XDG_DATA_HOME/grit-work` (normalt `~/.local/share/grit-work`) på Linux och `~/Library/Application Support/grit-work` på macOS. På Unix begränsas katalogen till ägaren.

Avslutade dygn kontrolleras vid start och varje timme. Arkivet ligger under `<vald mapp>/grit-work/ÅÅÅÅ-MM-DD/` och innehåller bilder, aktivitetsdata med OCR, eventuell rå OCR-dygnslogg och SHA-256-manifest. Källfiler och aktivitetsrader tas bort först efter verifiering. Om arkivet saknas, inte kan skrivas eller är skadat behålls lokalt underlag. Rapporter kan genereras från det verifierade arkivet efter lokal rensning. Rapporter och inställningar behålls lokalt.

Arkivplatsen låses efter första arkiverade dygnet; flytt av befintligt arkiv stöds ännu inte. Använd en egen arkivrot per installation. Automatisk rensning i originalappens gamla storleks-/åldershantering är avstängd.

## Rapporternas skydd

- Agenten får bilder, aktivitetsloggar och projektregister, aldrig Spiris-hemligheten. Underlaget kan innehålla känslig skärminformation och skickas till vald AI-leverantör när användaren startar rapportjobbet.
- Jobben körs med tidsgräns, avbrytning och begränsad svarsstorlek. Dygnet delas i tvåtimmarsintervall; mer än 64 bilder i ett intervall ger ett tydligt fel i stället för tyst trunkering.
- CLI-inställningar begränsar verktygen. Codex får bilder direkt, Grok måste anropa bildläsaren, agy läser arbetskatalogens bilder. Detta är inte en generell säkerhetsisolering från en komprometterad CLI.
- Backend validerar schemastruktur, verkliga underlagsreferenser, tidsgränser, överlapp och att minuterna ryms inom registrerad tid. Alla AI-rader kräver granskning.
- Före överföring hämtas projektregistret på nytt och projektets status och användarens medlemskap kontrolleras. Ändrade projektnamn eller nummer kräver ny granskning.
- Befintlig tid i Spiris för samma användare/dag blockerar överföringen. Den första versionen försöker inte slå ihop manuell tid och Grit-tid.
- Varje avsikt att skicka sparas före anropet. Vid avbrott eller osäkert svar låses rapporten för avstämning. Inga automatiska omförsök av tidsrader. Delvis överförda rapporter kräver manuell avstämning om alla rader inte kan verifieras.

Originalets övriga vyer finns kvar. Grit-flödet är på svenska, originalvyerna använder befintliga översättningar med engelska som standard. Automatisk uppdatering är avstängd tills forken har en egen signerad releasekedja.

## Utveckling och verifiering

Node 22+, Rust och Tauri 2:s plattformsberoenden behövs. Linux kräver WebKitGTK 4.1, GTK 3, Secret Service samt skärm-/fönsterverktyg för aktuell skrivbordsmiljö (på Hyprland bland annat `grim` och `hyprctl`).

```sh
npm ci
npm run verify:frontend
cargo test -p work-review --bin grit-work grit -- --test-threads=1
cargo test -p work-review-core
npm run tauri:build -- --bundles deb
```

På Arch/Omarchy kan den färdigbyggda binären paketeras med `cd packaging/arch && makepkg`; installera paketet med `sudo pacman -U grit-work-*.pkg.tar.zst`. Det ger appen en post i programstartaren. Debian-paketet hamnar under `target/release/bundle/deb/`.

Kör `npm run dev -- --host 127.0.0.1 --port 5182`, sedan `node scripts/test-grit-ui.mjs` för webbläsartestet med syntetiskt underlag. `CHROMIUM_PATH` och `GRIT_TEST_URL` kan sättas. Testet kontrollerar att Spiris-anropet inte sker före bekräftelsen och tar bilder i systemets temporärkatalog.

CLI-bildtestet körs med `cargo run -p work-review --example grit-agent-check -- PROVIDER MODEL IMAGE`, där den syntetiska bilden ska innehålla `SAPPHIRE-729`. Använd Herdr-panel för agentkörningar. `spiris-check` är ett separat läsdiagnostikverktyg som använder nyckellagret; det skickar aldrig tid.

Linux är byggtestad och skrivbordsstarten verifierad. Rapportvyn är interaktionstestad i Chromium med syntetiska data. Windows/macOS har plattformskod men är inte verifierade i denna iteration. Se [ursprunglig dokumentation](docs/grit/UPSTREAM-README.md) för Work Reviews övriga funktioner. Ursprunglig MIT-licens och upphovsrätt behålls.
