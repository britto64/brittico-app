; O instalador do Brittico para Windows (Inno Setup 6).
;
; Instala só para quem está usando o computador (em %LOCALAPPDATA%\Programs\Brittico): não pede
; administrador, e atualizar é rodar o instalador novo por cima. Pergunta se cria o atalho na área de
; trabalho e se o Brittico abre junto com o Windows (minimizado). Desinstalar tira os dois.
;
; Gerado pelo GitHub Actions (.github/workflows/build.yml):
;   iscc /DVersao=1.0.0 /DExe=..\..\target\release\brittico.exe packaging\windows\brittico.iss

#ifndef Versao
  #define Versao "0.0.0"
#endif
#ifndef Exe
  #define Exe "..\..\target\release\brittico.exe"
#endif

[Setup]
AppId={{6B0F3C1E-7A52-4C55-9B47-8E2D6C1B2F11}
AppName=Brittico
AppVersion={#Versao}
AppVerName=Brittico {#Versao}
AppPublisher=Brittico
AppPublisherURL=https://brittico.xyz/app/
AppSupportURL=https://github.com/britto64/brittico-app/issues
AppUpdatesURL=https://github.com/britto64/brittico-app/releases
DefaultDirName={autopf}\Brittico
DefaultGroupName=Brittico
DisableProgramGroupPage=yes
DisableWelcomePage=no
DisableDirPage=yes
DisableReadyPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir=..\..\dist
OutputBaseFilename=Brittico-Setup-x64
SetupIconFile=..\icons\brittico.ico
UninstallDisplayIcon={app}\brittico.exe
UninstallDisplayName=Brittico
WizardStyle=modern
WizardImageFile=assistente-grande.bmp,assistente-grande-2x.bmp
WizardSmallImageFile=assistente-pequena.bmp,assistente-pequena-2x.bmp
Compression=lzma2/ultra64
SolidCompression=yes
CloseApplications=yes
RestartApplications=no
VersionInfoVersion={#Versao}
VersionInfoProductName=Brittico
VersionInfoDescription=Instalador do Brittico

[Languages]
Name: "pt"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Messages]
pt.WelcomeLabel1=Bem-vindo ao Brittico
pt.WelcomeLabel2=O feed, a call e o LoL de proximidade num app só.%n%nIsto vai instalar o Brittico {#Versao} no seu computador. Leva poucos segundos e não precisa de administrador.

[CustomMessages]
pt.TarefaAtalho=Criar um atalho na área de trabalho
pt.TarefaIniciar=Abrir o Brittico junto com o Windows (minimizado)
pt.GrupoAtalhos=Atalhos:
pt.GrupoIniciar=Ao ligar o computador:
pt.AbrirAgora=Abrir o Brittico agora

[Tasks]
Name: "atalho"; Description: "{cm:TarefaAtalho}"; GroupDescription: "{cm:GrupoAtalhos}"
Name: "iniciar"; Description: "{cm:TarefaIniciar}"; GroupDescription: "{cm:GrupoIniciar}"; Flags: unchecked

[Files]
Source: "{#Exe}"; DestDir: "{app}"; DestName: "brittico.exe"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Brittico"; Filename: "{app}\brittico.exe"
Name: "{autodesktop}\Brittico"; Filename: "{app}\brittico.exe"; Tasks: atalho

[Registry]
; "Abrir junto com o Windows": a entrada de inicialização do próprio usuário, apagada ao desinstalar.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Brittico"; \
  ValueData: """{app}\brittico.exe"" --minimizado"; Flags: uninsdeletevalue; Tasks: iniciar

[Run]
Filename: "{app}\brittico.exe"; Description: "{cm:AbrirAgora}"; Flags: nowait postinstall skipifsilent

[Code]
// Reinstalar sem marcar "abrir com o Windows" tira a entrada que uma instalação antiga tinha posto.
procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and not WizardIsTaskSelected('iniciar') then
    RegDeleteValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Run', 'Brittico');
end;
