/=  mine  /common/pow
/=  nv  /common/nock-verifier
/=  *  /common/zoon
/=  *  /common/zeke
/=  *  /common/wrapper
::
=<  ((moat |) inner)
=>
|%
+$  kernel-state  ~
+$  mine-success
  $:  %command
      %pow
      =proof
      dig=tip5-hash-atom
      header=noun-digest:tip5
      nonce=noun-digest:tip5
  ==
+$  cause
  $%  $:  %verify
          res=mine-success
          target=bignum:bignum
          pow-len=@
  ==  ==
+$  effect
  $%  [%good ~]
      [%bad why=@tas]
  ==
--
::
|%
++  moat  (keep kernel-state)
++  inner
  |_  k=kernel-state
  ++  load  |=(=kernel-state kernel-state)
  ::
  ++  peek
    |=  arg=*
    =/  pax  ((soft path) arg)
    ?~  pax  ~|(not-a-path+arg !!)
    ~|(invalid-peek+pax !!)
  ::
  ++  poke
    |=  [wir=wire eny=@ our=@ux now=@da dat=*]
    ^-  [(list effect) k=kernel-state]
    ::
    =/  cause  ((soft cause) dat)
    ?~  cause
      ~>  %slog.[0 [%leaf "error: bad cause"]]
      `k
    =/  cause  u.cause
    ?>  ?=([%verify *] cause)
    =/  prf=proof  proof.res.cause
    ::
    ?:  =((lent objects.prf) 0)
      :_  k
      [%bad %invalid-proof]~
    ::
    =/  puzzle  (snag 0 objects.prf)
    ?.  ?=([%puzzle *] puzzle)
      :_  k
      [%bad %invalid-type]~
    ::
    ::  validate that the correct powork puzzle was solved
    =/  check-pow-puzzle=?
      =(pow-len.cause len.puzzle)
    ?.  check-pow-puzzle
      :_  k
      [%bad %wrong-pow-length]~
    ::
    ::  validate block commitment
    ?.  =(header.res.cause commitment.puzzle)
      :_  k
      [%bad %wrong-commitment]~    ::
    ::
    ::  validate the proof
    =/  valid=?
      (verify:nv prf ~ eny)
    ?.  valid
      :_  k
      [%bad %bad-proof]~
    ::
    ?:  (check-target:mine dig.res.cause target.cause)
      ::  they hit the target, credit the share
      :_  k
      [%good ~]~
    ::  they sent in a valid share that did not hit the target
    :_  k
    [%bad %below-target]~
  --
--
