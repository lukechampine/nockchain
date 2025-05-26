/=  z  /common/zeke
/=  *  /common/wrapper
=<  ((moat |) inner)  :: wrapped kernel
=>
  |%
  +$  effect  [%jojo res=vase print=@t]
  +$  kernel-state  [%state version=%1]
  +$  cause
    :: $+  cause
    $%  [%raw hoon=@t]
        [%sam function-name=@t sample=*]
        [%prt val=vase]
    ==
  --
|%
++  moat  (keep kernel-state) :: no state
++  inner
  |_  k=kernel-state
  ::  do-nothing load
  ++  load
    |=  =kernel-state  kernel-state
  ::  crash-only peek
  ++  peek
    |=  arg=*
    =/  pax  ((soft path) arg)
    ?~  pax  ~|(not-a-path+arg !!)
    ~|(invalid-peek+pax !!)
  ::  poke: try to prove a block
  ++  poke
    |=  [wir=wire eny=@ our=@ux now=@da dat=*]
    ^-  [(list effect) k=kernel-state]
    |^
    =/  cause  ((soft cause) dat)
    ?~  cause
      ~&  dat
      ~>  %slog.[0 [%leaf "error: bad cause"]]
      `k
    =/  cause  u.cause
    =/  res
      ?-  -.cause
        %sam  (do-sam function-name.cause sample.cause)
        %raw  (do-raw hoon.cause)
        %prt  val.cause
      ==
    =/  print  (crip (noah res))
    :_  k
      [%jojo res print]~
    ++  do-sam
        |=  [function-name=@t sample=*]
        =/  vas  (slap !>(z) (ream function-name))
        =/  res  (slym vas sample)
        res
    ++  do-raw
        |=  hoon=@t
        =/  res  (slap !>(z) (ream hoon))
        res
    --
  --
--
