; Reglas del Firewall de Windows para Boveda (#100). Segunda capa: el servidor ya
; solo escucha en redes que Windows marca como privadas; estas reglas permiten
; la app UNICAMENTE en el perfil privado.
;
; La instalacion sigue siendo por usuario. Solo el comando del Firewall se
; eleva (un unico aviso de UAC, con el verbo "runas"). Si se rechaza el UAC, la
; instalacion continua sin reglas y se avisa.

!define BOVEDA_RULE_TCP "Boveda servidor (red privada)"
!define BOVEDA_RULE_MDNS "Boveda mDNS (red privada)"

!macro NSIS_HOOK_POSTINSTALL
  ; Se borran primero las reglas anteriores (si las hay) para no dejar
  ; duplicados con otra ruta tras una reinstalacion.
  ExecShellWait "runas" "$SYSDIR\cmd.exe" '/c netsh advfirewall firewall delete rule name="${BOVEDA_RULE_TCP}" & netsh advfirewall firewall delete rule name="${BOVEDA_RULE_MDNS}" & netsh advfirewall firewall add rule name="${BOVEDA_RULE_TCP}" dir=in action=allow program="$INSTDIR\${MAINBINARYNAME}.exe" profile=private protocol=TCP enable=yes && netsh advfirewall firewall add rule name="${BOVEDA_RULE_MDNS}" dir=in action=allow program="$INSTDIR\${MAINBINARYNAME}.exe" profile=private protocol=UDP localport=5353 enable=yes' SW_HIDE
  IfErrors boveda_firewall_failed boveda_firewall_done
  boveda_firewall_failed:
    IfSilent boveda_firewall_done
    MessageBox MB_OK|MB_ICONINFORMATION "No se crearon las reglas del Firewall (el permiso de administrador no se concedio). Boveda funcionara, pero Windows puede pedir permiso al recibir conexiones. Puedes crearlas luego con los comandos de desktop/README.md."
  boveda_firewall_done:
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ExecShellWait "runas" "$SYSDIR\cmd.exe" '/c netsh advfirewall firewall delete rule name="${BOVEDA_RULE_TCP}" & netsh advfirewall firewall delete rule name="${BOVEDA_RULE_MDNS}"' SW_HIDE
!macroend
