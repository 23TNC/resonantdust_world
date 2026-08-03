; Needs & moodlets (work 2026-08-03-needs-moodlets) — the pawn's hidden state and
; its DISPLAYED consequences.
;
; A <need> is a 0..1 SATISFACTION that depletes toward zero (F1: one dialect for
; every need — bad states are LOW; the player never sees the scalar, so its
; direction is pure implementation). Nothing ticks it: a pawn's shard row stores
; (satisfaction, set_tic) and every observer computes the current value from
; `deplete` — TICS from full to empty (F4, the TICS/TILE posture).
;
;   &need.label            display/debug label (the need itself is never shown)
;   &need.deplete          TICS full→empty
;   &need.band.<i>.moodlet the <moodlet> active while lo <= satisfaction < hi
;   &need.band.<i>.lo/.hi  the band, EXCLUSIVE ranges — at most one active per need
;
; A <moodlet> is what the player sees (Sims-4 posture): a label + a mood offset.
; `duration 0` (default) = CONDITIONAL — alive exactly while its band holds,
; DERIVED by every observer, never granted (F2). `duration > 0` = TIMED — a
; stored grant expiring that many tics after it lands (the action stream's kind:
; drink → quenched).
;
;   &moodlet.label         the displayed name
;   &moodlet.mood          mood offset while active, -1..1 (mood = clamp(0.5 + Σ))
;   &moodlet.duration      TICS a stored grant lives; 0 = conditional
;
; A kind opts into needs in its :data @define: `"thirst &thing.needs.0 set`
; (content/data/things.rd — the wolf carries thirst).
;
; Numbers are PROVISIONAL and authored here precisely so tuning is a corpus edit.
; 21600 tics = 1 h wall at 6 Hz.

<need>
    ::thirst>
        @define>
            "Thirst &need.label set
            21600 &need.deplete set
            ; [0.10, 0.35) → Thirsty; [0, 0.10) → Dehydrated. Exclusive bands:
            ; a wolf is one or the other, never both.
            "thirsty &need.band.0.moodlet set
            0.10 &need.band.0.lo set
            0.35 &need.band.0.hi set
            "dehydrated &need.band.1.moodlet set
            0 &need.band.1.lo set
            0.10 &need.band.1.hi set
            0 return

<moodlet>
    ::thirsty>
        @define>
            "Thirsty &moodlet.label set
            -0.15 &moodlet.mood set
            0 return
    ::dehydrated>
        @define>
            "Dehydrated &moodlet.label set
            -0.40 &moodlet.mood set
            0 return
    ; Timed (duration > 0): granted by the drink action (the successor stream);
    ; authored now so the timed form is exercised by the loader from day one.
    ::quenched>
        @define>
            "Quenched &moodlet.label set
            0.20 &moodlet.mood set
            3600 &moodlet.duration set
            0 return
