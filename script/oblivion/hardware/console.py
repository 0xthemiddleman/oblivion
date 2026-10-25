#!/usr/bin/python3
"""Fixed local operator menu for the separate hardware-test distribution."""
from pathlib import Path
import subprocess
from ssh_control import addresses

UNIT = "oblivion-hardware-ssh.service"
notice = ""
while True:
    active = subprocess.run(["systemctl", "is-active", "--quiet", UNIT]).returncode == 0
    print("\033[0m\033[2J\033[H\033[95mOBLIVION - COLLAUDO HARDWARE\033[0m\n")
    print("Alt+F1: manager   |   Alt+F2: rete e collaudo\n")
    print("Immagine di test. Accesso amministrativo solo con la chiave del Mac.")
    print("Wi-Fi, impostazioni e diagnostica restano in RAM fino allo spegnimento.\n")
    for address in addresses():
        print("IP locale:", address.ip)
        if active:
            print("Utente SSH: oblivion-test (chiave dedicata sul Mac)")
    print("SSH: " + ("ATTIVO" if active else "SPENTO"))
    key = Path("/run/oblivion-hardware/ssh_host_ed25519_key.pub")
    if active and key.exists():
        print("Impronta del PC (da verificare sul Mac):")
        print(subprocess.check_output(["ssh-keygen", "-lf", str(key)], text=True).strip())
    print("\n1  Configura Wi-Fi / Ethernet\n2  Abilita accesso dal Mac\n3  Disabilita accesso dal Mac\n4  Mostra hardware\n5  Spegni\nInvio  Aggiorna stato\n")
    print(notice)
    choice = input("Scelta: ").strip()
    notice = ""
    if choice == "1":
        subprocess.run(["nmtui-connect"])
    elif choice in ["2", "3"]:
        result = subprocess.run(["systemctl", "start" if choice == "2" else "stop", UNIT], capture_output=True)
        notice = "Operazione completata." if result.returncode == 0 else "Operazione non riuscita. Verifica la connessione alla rete locale."
    elif choice == "4":
        subprocess.run(["lscpu"])
        subprocess.run(["lspci", "-nn"])
        print("KVM disponibile:", Path("/dev/kvm").exists())
        input("Invio per tornare al menu: ")
    elif choice == "5":
        subprocess.run(["systemctl", "poweroff"], check=True)
        break
