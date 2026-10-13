; Telex's native installer style, loaded by Tauri before its standard pages.
; Keep Tauri's installation, WebView2, maintenance and /P /UPDATE handling.
; Palette: ui/themes.css, light. Artwork: npm run icons --prefix site.

SetFont "Segoe UI" 9
Caption "$(TelexSetupCaption)"
InstallColors 1A1E23 F3F5F8
!define MUI_BGCOLOR "FFFFFF"
!define MUI_TEXTCOLOR "1A1E23"
!define MUI_HEADERIMAGE_RIGHT
!define MUI_WELCOMEFINISHPAGE_BITMAP_STRETCH "FitControl"
!define MUI_HEADERIMAGE_BITMAP_STRETCH "AspectFitHeight"
!define MUI_INSTFILESPAGE_COLORS "1A1E23 F3F5F8"
!define MUI_WELCOMEPAGE_TITLE "$(TelexWelcomeTitle)"
!define MUI_WELCOMEPAGE_TEXT "$(TelexWelcomeText)"
!define MUI_FINISHPAGE_TITLE "$(TelexFinishTitle)"
!define MUI_FINISHPAGE_TEXT "$(TelexFinishText)"
!define MUI_CUSTOMFUNCTION_GUIINIT TelexGuiInit
!define MUI_CUSTOMFUNCTION_UNGUIINIT un.TelexGuiInit

; Numeric IDs let these strings precede Tauri's MUI_LANGUAGE macros.
LangString TelexWelcomeTitle 1033 "Install Telex"
LangString TelexWelcomeTitle 1045 "Zainstaluj Telexa"
LangString TelexWelcomeText 1033 "Your mail, on your computer.$\r$\n$\r$\nA compact, keyboard-driven email client.$\r$\n$\r$\nChoose where to install Telex, then you're ready to add your accounts."
LangString TelexWelcomeText 1045 "Twoja poczta na Twoim komputerze.$\r$\n$\r$\nZwięzły klient poczty obsługiwany klawiaturą.$\r$\n$\r$\nWybierz miejsce instalacji Telexa, a potem dodaj swoje konta."
LangString TelexFinishTitle 1033 "Telex is ready"
LangString TelexFinishTitle 1045 "Telex jest gotowy"
LangString TelexFinishText 1033 "Open Telex to get started.$\r$\n$\r$\nYour mail and settings stay on this computer."
LangString TelexFinishText 1045 "Otwórz Telexa, aby zacząć.$\r$\n$\r$\nPoczta i ustawienia pozostają na tym komputerze."
LangString TelexSetupCaption 1033 "Telex · Setup"
LangString TelexSetupCaption 1045 "Telex · Instalacja"
LangString TelexUpdateCaption 1033 "Telex · Update"
LangString TelexUpdateCaption 1045 "Telex · Aktualizacja"
LangString TelexUpdating 1033 "Updating Telex"
LangString TelexUpdating 1045 "Aktualizowanie Telexa"
LangString TelexRestarting 1033 "Telex will open again when the update is ready."
LangString TelexRestarting 1045 "Telex otworzy się ponownie po aktualizacji."

Function TelexGuiInit
  Push $0
  SetCtlColors $HWNDPARENT "1A1E23" "F3F5F8"
  GetDlgItem $0 $HWNDPARENT 1256
  SetCtlColors $0 "59636E" "F3F5F8"
  SendMessage $0 ${WM_SETTEXT} 0 "STR:telexmail.app"
  Pop $0
FunctionEnd

Function un.TelexGuiInit
  SetCtlColors $HWNDPARENT "1A1E23" "F3F5F8"
FunctionEnd

!macro NSIS_HOOK_PREINSTALL
  ${If} $UpdateMode = 1
    SendMessage $HWNDPARENT ${WM_SETTEXT} 0 "STR:$(TelexUpdateCaption)"
    !insertmacro MUI_HEADER_TEXT "$(TelexUpdating)" "$(TelexRestarting)"
  ${EndIf}
!macroend
