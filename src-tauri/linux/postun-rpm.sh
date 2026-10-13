#!/bin/sh
# RPM %postun: $1 is 0 on erase, 1 or more on upgrade. On erase delete the
# per-user "Start at sign-in" entries RightPeek created.
if [ "$1" = "0" ]; then
  for entry in /root/.config/autostart/RightPeek.desktop /home/*/.config/autostart/RightPeek.desktop; do
    if [ -f "$entry" ] && grep -q '^Exec=/usr/bin/rightpeek' "$entry"; then
      rm -f "$entry"
    fi
  done
fi
exit 0
