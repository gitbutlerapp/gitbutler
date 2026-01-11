import type { DefineLocaleMessage } from '$lib/i18n/i18nLocale';

const locale: DefineLocaleMessage = {
	welcome: {
		auth: {
			title: 'Anmelden oder Registrieren',
			description:
				'Melde dich an, um auf intelligente Automatisierungsfunktionen zuzugreifen, einschließlich intelligenter Branch-Erstellung und Generierung von Commit-Nachrichten.',
			loginButton: 'Anmelden / Registrieren',
			cancel: 'Abbrechen',
			copyLoginLink: 'Login-Link kopieren'
		}
	},
	settings: {
		general: {
			title: 'Globale Einstellungen',
			general: {
				label: 'Allgemein',
				signingOut: {
					title: 'Abmelden',
					caption: 'Zeit für eine Pause? Klicke hier, um dich abzumelden und zu entspannen.',
					button: 'Abmelden'
				},
				language: {
					title: 'Sprache',
					changeSuccess: 'Sprache erfolgreich geändert'
				},
				codeEditor: {
					title: 'Standard-Code-Editor'
				},
				autoUpdate: {
					title: 'Automatisch nach Updates suchen',
					caption: 'Automatisch nach Updates suchen. Du kannst bei Bedarf auch manuell suchen.'
				},
				cliInstall: {
					title: 'GitButler CLI <code class="code-string">but</code> installieren',
					captionPackageManager:
						'Die <code>but</code> CLI wird von deinem Paketmanager verwaltet. Bitte verwende deinen Paketmanager zum Installieren, Aktualisieren oder Entfernen.',
					captionWindows:
						'Unter Windows kannst du die ausführbare Datei (<code>`but`</code>) manuell in ein Verzeichnis in deinem PATH kopieren. Klicke auf "Befehl anzeigen" für Anweisungen.',
					captionUnix:
						'Installiert die GitButler CLI (<code>`but`</code>) in deinem PATH, sodass du sie vom Terminal aus verwenden kannst. Diese Aktion erfordert Admin-Rechte. Alternativ könntest du auch manuell einen Symlink erstellen.',
					installButton: 'But CLI installieren',
					showCommandButton: 'Befehl anzeigen'
				},
				removeProjects: {
					title: 'Alle Projekte entfernen',
					caption:
						'Du kannst alle Projekte aus der GitButler-App löschen.<br />Dein Code bleibt sicher. Es wird nur die Konfiguration gelöscht.',
					button: 'Projekte entfernen…',
					modalTitle: 'Alle Projekte entfernen',
					modalMessage: 'Bist du sicher, dass du alle GitButler-Projekte entfernen möchtest?',
					removeButton: 'Entfernen',
					cancelButton: 'Abbrechen',
					success: 'Alle Konfigurationsdaten gelöscht',
					errorFailedDelete: 'Projekt konnte nicht gelöscht werden'
				},
				profileUpdate: {
					fullName: 'Vollständiger Name',
					email: 'E-Mail',
					updateButton: 'Profil aktualisieren',
					success: 'Profil aktualisiert',
					errorFailedUpdate: 'Benutzer konnte nicht aktualisiert werden',
					errorInvalidFile: 'Bitte verwende eine gültige Bilddatei'
				}
			},
			appearance: {
				label: 'Erscheinungsbild',
				theme: {
					title: 'Theme',
					light: 'Hell',
					dark: 'Dunkel',
					system: 'Systemeinstellung'
				},
				fileListMode: {
					title: 'Standard-Dateilistenansicht',
					caption: 'Setze die Standard-Dateilistenansicht (kann pro Standort geändert werden).',
					listView: 'Listenansicht',
					treeView: 'Baumansicht'
				},
				filePathFirst: {
					title: 'Dateipfad zuerst',
					caption: 'Zeige den vollständigen Dateipfad vor dem Dateinamen in Dateilisten an.'
				},
				diffPreview: {
					title: 'Diff-Vorschau'
				},
				diffFont: {
					title: 'Schriftart',
					caption:
						'Legt die Schriftart für die Diff-Ansicht fest. Der erste Schriftname ist der Standard, andere sind Ausweichmöglichkeiten.'
				},
				diffLigatures: {
					title: 'Schrift-Ligaturen erlauben'
				},
				tabSize: {
					title: 'Tab-Größe',
					caption: 'Anzahl der Leerzeichen pro Tab in der Diff-Ansicht.'
				},
				softWrap: {
					title: 'Weicher Umbruch',
					caption: 'Lange Zeilen in der Diff-Ansicht weich umbrechen, um in den Viewport zu passen.'
				},
				linesContrast: {
					title: 'Zeilenkontrast',
					caption: 'Der Kontrast für hinzugefügte, gelöschte und Kontextzeilen in Diffs.',
					light: 'Hell',
					medium: 'Mittel',
					strong: 'Stark'
				},
				colorBlindFriendly: {
					title: 'Farbsehschwäche-freundliche Farben',
					caption:
						'Verwende Blau und Orange statt Grün und Rot für bessere<br />Zugänglichkeit bei Farbsehschwäche.'
				},
				inlineWordDiffs: {
					title: 'Wort-Diffs inline anzeigen',
					caption:
						'Anstelle von separaten Zeilen für Entfernungen und Hinzufügungen zeigt diese Funktion eine einzelne Zeile mit hervorgehobenen hinzugefügten und entfernten Wörtern.'
				},
				scrollbarOnScroll: {
					title: 'Scrollbar-beim-Scrollen',
					caption: 'Zeige die Scrollbar nur beim Scrollen an.'
				},
				scrollbarOnHover: {
					title: 'Scrollbar-beim-Hover',
					caption: 'Zeige die Scrollbar nur an, wenn du über den scrollbaren Bereich fährst.'
				},
				scrollbarAlways: {
					title: 'Scrollbar immer anzeigen'
				},
				stagingBehavior: {
					stageAll: {
						title: 'Alle Dateien stagen',
						caption:
							'Stage alle dem Stack zugewiesenen Dateien beim Commit. Wenn keine Dateien gestaged sind, werden alle nicht zugewiesenen Dateien gestaged.'
					},
					stageSelection: {
						title: 'Ausgewählte Dateien stagen',
						caption:
							'Stage die ausgewählten zugewiesenen Dateien zum Stack beim Commit. Wenn keine Dateien ausgewählt sind, stage alle Dateien. Wenn es keine zugewiesenen Dateien gibt, stage alle ausgewählten nicht zugewiesenen Dateien.<br />Und wenn keine Dateien ausgewählt sind, stage alle nicht zugewiesenen Dateien.'
					},
					stageNone: {
						title: 'Dateien nicht automatisch stagen',
						caption:
							'Keine Dateien automatisch stagen.<br />Für Entwickler, die lieber selbst Hand anlegen.'
					}
				}
			},
			lanesAndBranches: {
				label: 'Lanes & Branches',
				newLanesPlacement: {
					title: 'Neue Lanes auf der linken Seite platzieren',
					caption:
						'Standardmäßig werden neue Lanes ganz rechts hinzugefügt. Aktiviere dies, um sie stattdessen ganz links hinzuzufügen.'
				},
				autoSelectCreation: {
					title: 'Text bei Branch-Erstellung automatisch auswählen',
					caption:
						'Wähle automatisch den vorausgefüllten Text im Branch-Namensfeld aus, wenn du einen neuen Branch erstellst, um das Eingeben eines eigenen Namens zu erleichtern.'
				},
				autoSelectRename: {
					title: 'Text bei Branch-Umbenennung automatisch auswählen',
					caption:
						'Wähle den Text automatisch aus, wenn du einen Branch oder eine Lane umbenennst, um das Ersetzen des gesamten Namens zu erleichtern.'
				}
			},
			git: {
				label: 'Git-Zeug',
				committerCredit: {
					title: 'GitButler als Committer angeben',
					caption:
						'Standardmäßig ist alles im GitButler-Client kostenlos nutzbar. Du kannst dich dafür entscheiden, uns als Committer in deinen virtuellen Branch-Commits anzugeben, um uns bekannter zu machen. <a href="https://github.com/gitbutlerapp/gitbutler-docs/blob/d81a23779302c55f8b20c75bf7842082815b4702/content/docs/features/virtual-branches/committer-mark.mdx">Mehr erfahren</a>'
				},
				autoFetch: {
					title: 'Auto-Fetch-Häufigkeit',
					oneMinute: '1 Minute',
					fiveMinutes: '5 Minuten',
					tenMinutes: '10 Minuten',
					fifteenMinutes: '15 Minuten',
					none: 'Keine'
				}
			},
			integrations: {
				label: 'Integrationen',
				autoFillPr: {
					title: 'PR/MR-Beschreibungen automatisch aus Commit ausfüllen',
					caption:
						'Beim Erstellen eines Pull Requests oder Merge Requests für einen Branch mit nur einem Commit wird die Nachricht dieses Commits automatisch als PR/MR-Titel und -Beschreibung verwendet.'
				},
				github: {
					authenticated: 'GitHub authentifiziert',
					authFailed: 'GitHub-Authentifizierung fehlgeschlagen',
					invalidToken: 'Ungültiges Token oder Netzwerkfehler',
					invalidTokenOrHost: 'Ungültiges Token oder Host',
					loadFailed: 'Laden der GitHub-Konten fehlgeschlagen',
					tryAgain: 'Erneut versuchen',
					caption: 'Ermöglicht das Erstellen von Pull Requests',
					copyCode: 'Kopiere den folgenden Verifizierungscode:',
					copyToClipboard: 'In Zwischenablage kopieren',
					navigateToGitHub:
						'Navigiere zur GitHub-Aktivierungsseite und füge den kopierten Code ein.',
					openGitHub: 'GitHub-Aktivierungsseite öffnen',
					checkStatus: 'Status prüfen',
					addPat: 'Personal Access Token hinzufügen',
					cancel: 'Abbrechen',
					addAccount: 'Konto hinzufügen',
					addAnotherAccount: 'Weiteres Konto hinzufügen',
					addGhe: 'GitHub Enterprise-Konto hinzufügen',
					gheCaption:
						'Um dich mit deiner GitHub Enterprise API zu verbinden, füge sie zur Whitelist in den CSP-Einstellungen der App hinzu.<br />Siehe <a href="https://docs.gitbutler.com/troubleshooting/custom-csp">Dokumentation für Details</a>',
					apiBaseUrl: 'API-Basis-URL',
					apiBaseUrlHelper:
						'Dies sollte die Stamm-URL der API sein. Wenn der Hostname deines GitHub Enterprise Servers beispielsweise github.acme-inc.com lautet, setze die Basis-URL auf https://github.acme-inc.com/api/v3',
					personalAccessToken: 'Personal Access Token',
					credentialsPersisted:
						'🔒 Anmeldeinformationen werden lokal in deinem OS Keychain / Credential Manager gespeichert.',
					authorizeAccount: 'GitHub-Konto autorisieren'
				}
			},
			ai: {
				label: 'KI-Optionen',
				about:
					'GitButler unterstützt mehrere KI-Anbieter: OpenAI und Anthropic (über API oder deinen eigenen Schlüssel), sowie lokale Modelle über Ollama und LM Studio.',
				useButlerApi: 'GitButler API verwenden',
				bringYourOwn: 'Eigener Schlüssel',
				openAi: {
					title: 'Open AI',
					keyPrompt: 'Möchtest du deinen eigenen Schlüssel angeben?',
					signInMessage: 'Bitte melde dich an, um die GitButler API zu verwenden.',
					butlerApiNote:
						'GitButler verwendet die OpenAI API für Commit-Nachrichten und Branch-Namen.',
					keyLabel: 'API-Schlüssel',
					modelVersion: 'Modellversion',
					customEndpoint: 'Benutzerdefinierter Endpunkt'
				},
				anthropic: {
					title: 'Anthropic',
					keyPrompt: 'Möchtest du deinen eigenen Schlüssel angeben?',
					signInMessage: 'Bitte melde dich an, um die GitButler API zu verwenden.',
					butlerApiNote:
						'GitButler verwendet die Anthropic API für Commit-Nachrichten und Branch-Namen.',
					keyLabel: 'API-Schlüssel',
					modelVersion: 'Modellversion'
				},
				ollama: {
					title: 'Ollama 🦙',
					configTitle: 'Ollama konfigurieren',
					configContent:
						'Um dich mit deinem Ollama-Endpunkt zu verbinden, <b>füge ihn zur Whitelist in den CSP-Einstellungen der App hinzu</b>.<br />Siehe die <a href="https://docs.gitbutler.com/troubleshooting/custom-csp">Dokumentation für Details</a>'
				},
				lmStudio: {
					title: 'LM Studio',
					endpoint: 'Endpunkt',
					model: 'Modell',
					configTitle: 'LM Studio konfigurieren',
					configContent:
						'<p>Die Verbindung zu deinem LM Studio-Endpunkt erfordert zwei Dinge:</p><p>1. <span class="text-bold">Füge ihn zur Whitelist in den CSP-Einstellungen der Anwendung hinzu</span>. Weitere Details findest du in der <a href="https://docs.gitbutler.com/troubleshooting/custom-csp">GitButler-Dokumentation</a>.</p><p>2. <span class="text-bold">Aktiviere CORS-Unterstützung in LM Studio</span>. Weitere Details findest du in der <a href="https://lmstudio.ai/docs/cli/server-start#enable-cors-support">LM Studio-Dokumentation</a>.</p>'
				},
				contextLength: {
					title: 'Umfang des bereitgestellten Kontexts',
					caption: 'Wie viele Zeichen deines Git-Diffs der KI bereitgestellt werden sollen'
				},
				customPrompts: {
					title: 'Benutzerdefinierte KI-Prompts',
					description:
						'GitButlers KI-Assistent generiert Commit-Nachrichten und Branch-Namen. Verwende Standard-Prompts oder erstelle eigene. Weise Prompts in den Projekteinstellungen zu.'
				},
				modelNames: {
					gpt5: 'GPT 5',
					gpt5Mini: 'GPT 5 Mini',
					o3Mini: 'o3 Mini',
					o1Mini: 'o1 Mini',
					gpt4oMini: 'GPT 4o mini',
					gpt41: 'GPT 4.1',
					gpt41Mini: 'GPT 4.1 mini (empfohlen)',
					haiku: 'Haiku',
					sonnet35: 'Sonnet 3.5',
					sonnet37: 'Sonnet 3.7 (empfohlen)',
					sonnet4: 'Sonnet 4',
					opus4: 'Opus 4'
				}
			},
			telemetry: {
				label: 'Telemetrie',
				description:
					'GitButler verwendet Telemetrie ausschließlich zur Verbesserung des Clients. Wir sammeln keine persönlichen Informationen, es sei denn, dies wird unten ausdrücklich erlaubt. <a href="https://gitbutler.com/privacy">Datenschutzrichtlinie</a>',
				request:
					'Wir würden uns freuen, wenn du diese Einstellungen aktiviert lässt, da sie uns helfen, Probleme schneller zu finden. Falls du sie deaktivierst, teile uns gerne dein Feedback auf unserem <a href="https://discord.gg/MmFkmaJ42D">Discord</a> mit.',
				errorReporting: {
					title: 'Fehlerberichterstattung',
					caption: 'Schalte die Meldung von Anwendungsabstürzen und Fehlern um.'
				},
				usageMetrics: {
					title: 'Nutzungsmetriken',
					caption: 'Schalte die Weitergabe von Nutzungsstatistiken um.'
				},
				nonAnonMetrics: {
					title: 'Nicht-anonyme Nutzungsmetriken',
					caption: 'Schalte die Weitergabe identifizierbarer Nutzungsstatistiken um.'
				}
			},
			experimental: {
				label: 'Experimentell',
				about:
					'Flags für Features in Entwicklung oder Beta. Features funktionieren möglicherweise nicht vollständig.<br />Verwendung auf eigenes Risiko.',
				apply3: {
					title: 'Neues Anwenden auf Workspace',
					caption:
						'Verwende die V3-Version der Apply- und Unapply-Operationen für Workspace-Änderungen.'
				},
				fMode: {
					title: 'F-Modus-Navigation',
					caption:
						'Aktiviere den F-Modus für schnelle Tastaturnavigation zu Schaltflächen mit Zwei-Buchstaben-Shortcuts.'
				},
				newRebase: {
					title: 'Neue Rebase-Engine',
					caption: 'Verwende die neue graphbasierte Rebase-Engine für Stack-Operationen.'
				},
				singleBranch: {
					title: 'Single-Branch-Modus',
					caption:
						'Bleibe in der Workspace-Ansicht, wenn du den gitbutler/workspace-Branch verlässt.'
				},
				irc: {
					title: 'IRC',
					caption: 'Aktiviere experimentellen In-App-Chat.',
					serverLabel: 'Server'
				}
			},
			organizations: {
				label: 'Organisationen',
				createButton: 'Neue Organisation erstellen'
			},
			footer: {
				social: {
					docs: 'Dokumentation',
					discord: 'Unser Discord'
				}
			}
		},
		project: {
			title: 'Projekteinstellungen',
			project: {
				label: 'Projekt'
			},
			git: {
				label: 'Git-Zeug',
				allowForcePush: {
					title: 'Force-Push erlauben',
					caption:
						'Force-Push ermöglicht es GitButler, Branches zu überschreiben, auch wenn sie bereits auf Remote gepusht wurden. GitButler wird niemals Force-Push auf den Zielbranch anwenden.'
				},
				forcePushProtection: {
					title: 'Force-Push-Schutz',
					caption:
						'Schütze Remote-Commits während Force-Pushs. Dies verwendet Gits sicherere Force-Push-Flags, um das Überschreiben der Remote-Commit-Historie zu vermeiden.'
				}
			},
			ai: {
				label: 'KI-Optionen',
				description:
					'GitButler unterstützt die Verwendung von OpenAI und Anthropic zur Generierung von Commit-Nachrichten und Branch-Namen. Dies funktioniert entweder über GitButlers API oder in einer Bring-Your-Own-Key-Konfiguration und kann im Haupteinstellungsbildschirm konfiguriert werden.',
				enableGeneration: {
					title: 'Branch- und Commit-Nachrichtengenerierung aktivieren',
					caption:
						'Wenn aktiviert, werden Diffs an die Server von OpenAI oder Anthropic gesendet, wenn die Schaltflächen "Nachricht generieren" und "Branch-Namen generieren" gedrückt werden.'
				},
				enableExperimental: {
					title: 'Experimentelle KI-Funktionen aktivieren',
					caption:
						'Wenn aktiviert, kannst du auf die derzeit in Entwicklung befindlichen KI-Funktionen zugreifen. Dies erfordert auch, dass du OpenAI über GitButler verwendest, damit die Funktionen funktionieren.'
				},
				customPrompts: {
					title: 'Benutzerdefinierte Prompts',
					description:
						'Du kannst eigene benutzerdefinierte Prompts auf das Projekt anwenden. Standardmäßig verwendet das Projekt GitButler-Prompts, aber du kannst in den allgemeinen Einstellungen eigene Prompts erstellen.',
					button: 'Prompts anpassen'
				}
			},
			agent: {
				label: 'Agent',
				guideText:
					'Den vollständigen Leitfaden zu Agents in GitButler findest du in <a href="https://docs.gitbutler.com/features/agents-tab#installing-claude-code">unserer Dokumentation</a>',
				autoCommit: {
					title: 'Automatisch committen nach Fertigstellung',
					caption:
						'Automatisch committen und Branches umbenennen, wenn Claude Code fertig ist. Deaktivieren, um vor dem Committen manuell zu überprüfen.'
				},
				useConfiguredModel: {
					title: 'Konfiguriertes Modell verwenden',
					caption: 'Verwende das in .claude/settings.json konfigurierte Modell.'
				},
				newlineOnEnter: {
					title: 'Zeilenumbruch bei Enter',
					caption: 'Verwende Enter für Zeilenumbrüche und Cmd+Enter zum Absenden.'
				},
				notifyOnCompletion: 'Benachrichtigen, wenn fertig',
				notifyOnPermissionRequest: 'Benachrichtigen, wenn Berechtigung benötigt wird',
				dangerousPermissions: {
					title: '⚠ Gefährlich: Alle Berechtigungen erlauben',
					caption:
						'Überspringt alle Berechtigungsaufforderungen und gewährt Claude Code uneingeschränkten Zugriff. Mit äußerster Vorsicht verwenden.'
				}
			},
			experimental: {
				label: 'Experimentell',
				ignoreCertificate: {
					title: 'Host-Zertifikatsprüfungen ignorieren',
					caption:
						'Die Aktivierung ignoriert Host-Zertifikatsprüfungen bei der Authentifizierung mit SSH.'
				}
			},
			details: {
				projectPath: 'Projektpfad',
				projectName: 'Projektname',
				projectNamePlaceholder: 'Projektname darf nicht leer sein',
				projectDescription: 'Projektbeschreibung',
				runGitHooks: {
					title: 'Git-Hooks ausführen',
					caption:
						'Wenn aktiviert, werden die in deinem Repository konfigurierten Git-pre-push-, pre- und post-commit- sowie commit-msg-Hooks ausgeführt.'
				}
			},
			disableCodegen: {
				title: 'Codegenerierung deaktivieren',
				caption: 'Verbirgt die Codegen-Schaltfläche in den Branch-Headern.'
			},
			baseBranch: {
				loading: 'Lade Remote-Branches...',
				title: 'Remote-Konfiguration',
				caption:
					"Ermöglicht die Auswahl, wohin Code gepusht werden soll, und das Festlegen des Zielbranches für Beiträge. Der Zielbranch ist normalerweise der \"Produktions\"-Branch wie 'origin/master' oder 'upstream/main'. Dieser Abschnitt hilft sicherzustellen, dass dein Code an den richtigen Remote und Branch für die Integration geht.",
				currentTargetBranch: 'Aktueller Zielbranch',
				createBranchesOnRemote: 'Branches auf Remote erstellen',
				activeBranchesWarning:
					'Du hast {count} aktiven Branch in deinem Workspace. Bitte räume den Workspace auf, bevor du den Basis-Branch wechselst. | Du hast {count} aktive Branches in deinem Workspace. Bitte räume den Workspace auf, bevor du den Basis-Branch wechselst.',
				switchingBranches: 'Wechsle Branches...',
				updateConfiguration: 'Konfiguration aktualisieren',
				errorListingBranches: 'Beim Auflisten deiner Remote-Branches ist ein Fehler aufgetreten'
			},
			remove: {
				title: 'Projekt entfernen',
				caption:
					'Beim Entfernen eines Projekts wird nur die Konfiguration gelöscht — dein Code bleibt sicher.',
				success: 'Projekt gelöscht',
				error: 'Projekt konnte nicht gelöscht werden'
			}
		},
		error: {
			notFound: 'Einstellungsseite {id} nicht gefunden.'
		}
	}
};

export default locale;
