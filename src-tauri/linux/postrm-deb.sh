#!/bin/sh
# Debian postrm: on removal (not upgrade) delete the per-user "Start at
# sign-in" entries RightPeek created, so no broken autostart entry is left.
case "$1" in
  remove|purge)
    for entry in /root/.config/autostart/RightPeek.desktop /home/*/.config/autostart/RightPeek.desktop; do
      if [ -f "$entry" ] && grep -q '^Exec=/usr/bin/rightpeek' "$entry"; then
        rm -f "$entry"
      fi
    done
    ;;
esac
exit 0
