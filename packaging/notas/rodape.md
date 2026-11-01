### Qual baixar

| Sistema | Arquivo |
|---|---|
| **Windows 10/11** | `Brittico-Setup-x64.exe` (instalador) ou `Brittico-portable-x64.exe` |
| **Qualquer Linux** | `Brittico-x86_64.AppImage` |
| **Arch, Manjaro** | `brittico-x86_64.pkg.tar.zst` (`sudo pacman -U ...`) |
| **Debian, Ubuntu, Mint** | `brittico_amd64.deb` (`sudo apt install ./...`) |
| **Fedora, openSUSE** | `brittico-x86_64.rpm` (`sudo dnf install ./...`) |
| **Outro Linux** | `brittico-linux-x86_64.tar.gz` |

O instalador do Windows ainda não é assinado: se aparecer "O Windows protegeu o computador", clique em
**Mais informações → Executar assim mesmo**. As somas estão em `SHA256SUMS.txt`, e cada arquivo tem atestação de origem
do GitHub (`gh attestation verify <arquivo> -R britto64/brittico-app`).
