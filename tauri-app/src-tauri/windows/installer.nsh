; src-tauri/windows/installer.nsh
; Custom NSIS hooks for Tauri + CEP extension (Tauri v2)
; Use Tauri's hook macros: https://v2.tauri.app/distribute/windows-installer/#nsis-installer-hooks


; Prompts the user to close any Adobe apps running, without actually checking if they are still running (would cause dependency problem)

; Upon reinstallation, any old CEP files would be overwritten by the new ones. 

!macro NSIS_HOOK_PREINSTALL
  ; --- Runs BEFORE the main installation begins ---
  ; Request admin privileges (required for writing to Program Files)
  SetShellVarContext all
  UserInfo::GetAccountType
  Pop $0
  ${If} $0 != "admin"
    MessageBox MB_ICONSTOP "Administrator rights are required to install the Adobe CEP extension."
    SetErrorLevel 740 ; ERROR_ELEVATION_REQUIRED
    Quit
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; --- Runs AFTER the main Tauri app files are copied ---
  ; Use stack to avoid variable conflicts
  Push $0
  Push $1
  
  ; Set source and target directories
  StrCpy $0 "$INSTDIR\resources\cep-extension\TauriApp_Client"  ; Source dir
  StrCpy $1 "C:\Program Files (x86)\Common Files\Adobe\CEP\extensions\TauriApp_Client"  ; Target dir

  ; Check if source exists
  IfFileExists "$0\*.*" source_exists 0
    DetailPrint "❌ CEP source not found: $0"
    Goto end_install
  source_exists:

  ; Create target dir if missing
  CreateDirectory "$1"

  ; Copy files
  DetailPrint "📂 Copying CEP extension files..."
  CopyFiles /SILENT "$0\*.*" "$1"

  DetailPrint "✅ CEP extension installed: $1"
  
  end_install:
  ; Restore preserved values
  Pop $1
  Pop $0
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; --- Runs BEFORE the main Tauri app files are removed ---
  ; Prompt user to close Adobe applications
  MessageBox MB_YESNO|MB_ICONEXCLAMATION \
    "Please ensure all Adobe applications (especially Premiere Pro) are completely closed before continuing.$\n$\n\
     This ensures CEP extensions can be properly removed.$\n$\n\
     Have you closed all Adobe applications?" \
    IDYES proceed_with_uninstall
    IDNO cancel_uninstall

  proceed_with_uninstall:
    ; Proceed with CEP removal
    Push $0
    StrCpy $0 "C:\Program Files (x86)\Common Files\Adobe\CEP\extensions\TauriApp_Client"
    
    ; Check if CEP extension exists
    IfFileExists "$0\*.*" cep_exists 0
      DetailPrint "⚠️ CEP extension not found at: $0"
      Goto end_uninstall
    cep_exists:
    
    ; Remove directory recursively
    DetailPrint "🗑️ Removing CEP extension: $0"
    RMDir /r "$0"
    DetailPrint "✅ CEP extension removed."
    Goto end_uninstall

  cancel_uninstall:
    MessageBox MB_ICONINFORMATION "Please close all Adobe applications and run the uninstaller again."
    SetErrorLevel 2 ; User cancelled
    Abort

  end_uninstall:
    Pop $0
!macroend
