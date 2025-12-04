/=  compute-table-v0-v1  /common/v0-v1/table/prover/compute
/=  compute-table-v2  /common/v2/table/prover/compute
/=  memory-table-v0-v1   /common/v0-v1/table/prover/memory
/=  memory-table-v2   /common/v2/table/prover/memory
/=  *  /common/zeke
/=  nock-common-v0-v1  /common/v0-v1/nock-common
/=  nock-common-v2     /common/v2/nock-common
::
=>  :*  stark-engine
        nock-common-v0-v1=nock-common-v0-v1
        nock-common-v2=nock-common-v2
        compute-table-v0-v1=compute-table-v0-v1
        compute-table-v2=compute-table-v2
        memory-table-v0-v1=memory-table-v0-v1
        memory-table-v2=memory-table-v2
    ==
~%  %stark-prover  ..stark-engine-jet-hook  ~
|%
+$  prover-input
  $%  $:  version=%0
          header=noun-digest:tip5
          nonce=noun-digest:tip5
          pow-len=@
      ==
  ::
      $:  version=%1
          header=noun-digest:tip5
          nonce=noun-digest:tip5
          pow-len=@
      ==
  ::
      $:  version=%2
          header=noun-digest:tip5
          nonce=noun-digest:tip5
          pow-len=@
      ==
  ==
::
+$  prove-result  (each =proof err=prove-err)
+$  prove-err     $%([%too-big heights=(list @)])
+$  prover-output    [=proof deep-codeword=fpoly]
::
::
::  +prove: prove the Nock computation [s f]
++  prove
  ~/  %prove
  |=  prover-input
  ^-  prove-result
  =/  [s=* f=*]  (puzzle-nock header nonce pow-len)
  =/  [prod=* return=fock-return]  (fink:fock [s f])
  =/  nock-common=_nock-common-v0-v1
    ?-  version
      %0  nock-common-v0-v1
      %1  nock-common-v0-v1
      %2  nock-common-v2
    ==
  =/  compute-funcs=table-funcs
    ?-  version
      %0  funcs:compute-table-v0-v1
      %1  funcs:compute-table-v0-v1
      %2  funcs:compute-table-v2
    ==
  =/  compute-common=static-table-common
    ?-  version
      %0  static:common:compute-table-v0-v1
      %1  static:common:compute-table-v0-v1
      %2  static:common:compute-table-v2
    ==
  =/  memory-funcs=table-funcs
    ?-  version
      %0  funcs:memory-table-v0-v1
      %1  funcs:memory-table-v0-v1
      %2  funcs:memory-table-v2
    ==
  =/  memory-common=static-table-common
    ?-  version
      %0  static:common:memory-table-v0-v1
      %1  static:common:memory-table-v0-v1
      %2  static:common:memory-table-v2
    ==
  =/  pre=preprocess-data
    ?-  version
      %0  p.pre-0-1.prep.stark-config
      %1  p.pre-0-1.prep.stark-config
      %2  p.pre-2.prep.stark-config
    ==
  %-  %~  generate-proof
        prove-door
      :*  nock-common
          compute-funcs
          compute-common
          memory-funcs
          memory-common
          pre
        ==
  [version header nonce pow-len s f prod return]
::
++  prove-door
  ~/  %prove-door
  |_  $:  nock-common=_nock-common-v0-v1
          compute-funcs=table-funcs
          compute-common=static-table-common
          memory-funcs=table-funcs
          memory-common=static-table-common
          pre=preprocess-data
      ==
  ::
  :: generate-proof is the main body of the prover.
  ++  generate-proof
    :: Disabled jet hint for now, under development.
    ~/  %generate-proof
    |=  $:  version=proof-version
            header=noun-digest:tip5
            nonce=noun-digest:tip5
            pow-len=@
            s=*
            f=*
            prod=*
            return=fock-return
        ==
    ^-  prove-result
    =|  =proof  ::  the proof stream
                                                                    =.  proof  (~(push proof-stream proof) [%puzzle header nonce pow-len prod])
    ::
    ::  build tables
    =/  tables=(list table-dat)  (build-table-dats return)
                                                                    =/  num-tables  (lent tables)
                                                                    =/  table-names  (turn tables |=(t=table-dat name.p.t))
                                                                    =/  heights  (table-heights tables)
                                                                    =.  proof  (~(push proof-stream proof) [%heights heights])
    =/  clc  ~(. calc heights cd.pre)
                                                                    =*  fri-domain-len=@  init-domain-len:fri:clc
                                                                    =/  [base-marys=(list mary) width=@]  (bas-mary tables)
                                                                    =/  base=codeword-commitments  (compute-codeword-commitments base-marys fri-domain-len width)
                                                                    =.  proof  (~(push proof-stream proof) [%m-root h.q.merk-heap.base])
                                                                    =/  chals-rd1  (make-chals proof num-chals-rd1:chal)
    ::
    ::  extension columns: list or mary? probably should be a list
    ::
    ::  build extension columns
    =/  table-exts=(list table-mary)
      %+  turn  tables
      |=  t=table-dat
      ^-  table-mary
      (extend:q.t p.t chals-rd1 return)
                                                                    =.  tables  (weld-table-marys tables table-exts)
                                                                    =/  [ext-marys=(list mary) width=@]  (ext-mary table-exts)
                                                                    =/  ext=codeword-commitments  (compute-codeword-commitments ext-marys fri-domain-len width)
                                                                    =.  proof  (~(push proof-stream proof) [%m-root h.q.merk-heap.ext])
                                                                    =/  chals-rd2  (make-chals proof num-chals-rd2:chal)
                                                                    =/  challenges  (weld chals-rd1 chals-rd2)
    ::
    =/  table-mega-exts=(list table-mary)  (build-mega-extend tables challenges return)
    =/  augmented-chals=bpoly  (augment-challenges:chal challenges s f)
    ::
                                                                    =.  tables  (weld-table-marys tables table-mega-exts)
                                                                    =/  [mega-ext-marys=(list mary) width=@]  (mega-ext-mary table-mega-exts)
                                                                    =/  mega-ext=codeword-commitments  (compute-codeword-commitments mega-ext-marys fri-domain-len width)
    ::
    ::  get terminal values for use in permutation/evaluation arguments
    =/  dyn-list=(list bpoly)
      %+  turn  tables
      |=  t=table-dat
      (terminal:q.t p.t)
    ::
                                                                    =/  terminals=bpoly  (weld-terminals dyn-list)
                                                                    =.  proof  (~(push proof-stream proof) terms+terminals)
                                                                    =/  trace-polys  (make-trace-polys polys.base polys.ext polys.mega-ext)
                                                                    =/  second-row-trace-polys=(list mary)  (make-second-row-trace-polys tables)
                                                                    =/  tworow-trace-polys=(list mary)
                                                                      %^    zip
                                                                          trace-polys
                                                                        second-row-trace-polys
                                                                      |=  [t-poly=mary s-poly=mary]
                                                                      (~(weld ave t-poly) s-poly)
                                                                    =/  max-constraint-degree  (get-max-constraint-degree cd.pre)
                                                                    =/  ntt-len  (bex (xeb (dec (get-max-constraint-degree cd.pre))))
                                                                    =/  max-height=@  (bex (xeb (dec (roll heights max))))
                                                                    =/  tworow-trace-polys-eval=(list bpoly)
                                                                      %+  turn  tworow-trace-polys
                                                                      |=  polys=mary
                                                                      (precompute-ntts polys max-height ntt-len)
                                                                    =/  [omicrons-bpoly=bpoly omicrons-fpoly=fpoly]  (make-omicrons tables)
                                                                    =/  extra-composition-poly=bpoly
                                                                      %-  make-composition-poly
                                                                      :*  proof
                                                                          omicrons-bpoly
                                                                          heights
                                                                          tworow-trace-polys-eval
                                                                          constraint-map.pre
                                                                          count-map.pre
                                                                          augmented-chals
                                                                          dyn-list
                                                                          %.y
                                                                      ==
                                                                    =.  proof  (~(push proof-stream proof) [%poly extra-composition-poly])
                                                                    =/  rng  ~(prover-fiat-shamir proof-stream proof)
                                                                    =^  extra-comp-eval-point  rng  $:felt:rng
                                                                    =/  extra-trace-evaluations=fpoly  (make-trace-evals tworow-trace-polys extra-comp-eval-point)
                                                                    =.  proof  (~(push proof-stream proof) [%evals extra-trace-evaluations])
                                                                    =.  proof  (~(push proof-stream proof) [%m-root h.q.merk-heap.mega-ext])
                                                                    =/  composition-poly=bpoly
                                                                      %-  make-composition-poly
                                                                      :*  proof
                                                                          omicrons-bpoly
                                                                          heights
                                                                          tworow-trace-polys-eval
                                                                          constraint-map.pre
                                                                          count-map.pre
                                                                          augmented-chals
                                                                          dyn-list
                                                                          %.n
                                                                      ==
                                                                    =/  num-composition-pieces  (get-max-constraint-degree cd.pre)
                                                                    =/  composition-pieces=(list bpoly)  (bp-decompose composition-poly num-composition-pieces)
                                                                    =/  composition-codewords=mary  (make-composition-codewords composition-pieces fri-domain-len)
                                                                    =/  composition-codeword-array=mary  (transpose-bpolys composition-codewords)
                                                                    =/  composition-merk  (bp-build-merk-heap:merkle composition-codeword-array)
                                                                    =.  proof  (~(push proof-stream proof) [%comp-m h.q.composition-merk num-composition-pieces])
                                                                    =/  deep-challenge=felt  (make-deep-challenge proof fri-domain-len)
                                                                    =/  trace-evaluations=fpoly  (make-trace-evals tworow-trace-polys deep-challenge)
    ::~&  %evaluating-pieces-at-deep-challenge
    =/  composition-pieces-fpoly  (turn composition-pieces bpoly-to-fpoly)
    =/  composition-piece-evaluations=fpoly  (make-composition-piece-evals deep-challenge composition-pieces-fpoly)
    ::
                                                                    =.  proof  (~(push proof-stream proof) [%evals trace-evaluations])
                                                                    =.  proof  (~(push proof-stream proof) [%evals composition-piece-evaluations])
                                                                    =/  deep-weights=fpoly  (make-deep-weights proof tables max-constraint-degree)
                                                                    =/  all-evals  (~(weld fop trace-evaluations) extra-trace-evaluations)
                                                                    =/  deep-poly=fpoly
                                                                      %-  compute-deep
                                                                      :*  trace-polys
                                                                          all-evals
                                                                          composition-pieces-fpoly
                                                                          composition-piece-evaluations
                                                                          deep-weights
                                                                          omicrons-fpoly
                                                                          deep-challenge
                                                                          extra-comp-eval-point
                                                                      ==
                                                                    =/  deep-codeword=fpoly  (coseword deep-poly (lift g) fri-domain-len)
    =^  fri-indices  proof  (prove:fri:clc deep-codeword proof)
                                                                    =/  composition-commitments  [~ composition-codeword-array composition-merk]
                                                                    =.  proof  (add-commitments proof fri-indices ~[base ext mega-ext composition-commitments])
    ::
    ::~&  %finished-proof
    ?-  version
      %0  [%& %0 objects.proof ~ 0]
      %1  [%& %1 objects.proof ~ 0]
      %2  [%& %2 objects.proof ~ 0]
    ==
  ::
  ::
  ++  build-table-dats
    ~/  %build-table-dats
    |=  return=fock-return
    ^-  (list table-dat)
    %-  sort
    :_  td-order
    %+  turn  gen-table-names:nock-common
    |=  name=term
    =/  t-funcs
      ~|  "table-funcs do not exist for {<name>}"
      (~(got by table-funcs-map) name)
    =/  v-funcs
      ~|  "verifier-funcs do not exist for {<name>}"
      (~(got by all-verifier-funcs-map:nock-common) name)
    =/  tm=table-mary  (build:t-funcs return)
    [(pad:t-funcs tm) t-funcs v-funcs]
  ::
  ++  table-funcs-map
    ~+
    ^-  (map term table-funcs)
    %-  ~(gas by *(map term table-funcs))
    :~  :-  name:compute-common
        compute-funcs
        :-  name:memory-common
        memory-funcs
    ==
  ++  build-mega-extend
    ~/  %build-mega-extend
    |=  [tables=(list table-dat) chals=(list belt) return=fock-return]
    ^-  (list table-mary)
    %+  turn  tables
    |=  t=table-dat
    ^-  table-mary
    (mega-extend:q.t p.t chals return)
  ::
  ::  interim jets
  ::
  ++  table-heights
    ~/  %table-heights
    |=  tables=(list table-dat)
    ^-  (list @)
    %+  turn  tables
    |=  t=table-dat
    =/  len  len.array.p.p.t
    ?:(=(len 0) 0 (bex (xeb (dec len))))
  ::
  ++  bas-mary
    ~/  %bas-mary
    |=  tables=(list table-dat)
    ^-  [marys=(list mary) width=@]
    %^  spin  tables
      0
    |=([t=table-dat width=@] [p.p.t (add width base-width.p.t)])
  ::
  ++  ext-mary
    ~/  %ext-mary
    |=  tables=(list table-mary)
    ^-  [marys=(list mary) width=@]
    %^  spin  tables
      0
    |=([t=table-mary width=@] [p.t (add width ext-width.t)])
  ::
  ++  mega-ext-mary
    ~/  %mega-ext-mary
    |=  table-mega-exts=(list table-mary)
    ^-  [marys=(list mary) width=@]
    %^  spin  table-mega-exts
      0
    |=([t=table-mary width=@] [p.t (add width mega-ext-width.t)])
  ::
  ++  make-chals
    ~/  %make-chals
    |=  [=proof num-chals=@]
    ^-  (list belt)
    =/  rng  ~(prover-fiat-shamir proof-stream proof)
    =^  chals=(list belt)  rng  (belts:rng num-chals)
    chals
  ::
  ++  weld-table-marys
    ~/  %weld-table-marys
    |=  [ts=(list table-dat) ms=(list table-mary)]
    ^-  (list table-dat)
    %+  turn  (zip-up ts ms)
    |=  [t=table-dat ext=table-mary]
    ^-  table-dat
    :_  [q.t r.t]
    (weld-exts:tlib p.t ext)
  ::
  ++  make-deep-weights
    ~/  %make-deep-weights
    |=  [=proof tables=(list table-dat) max-constraint-degree=@]
    =/  rng  ~(prover-fiat-shamir proof-stream proof)
    =/  total-cols=@
      %+  roll  tables
      |=  [[p=table-mary *] sum=@]
      (add sum step:p.p)
    =/  rng-max  (add (mul 4 total-cols) max-constraint-degree)
    =^  felt-list  rng  (felts:rng rng-max)
    (init-fpoly felt-list)
  ::
  ++  make-omicrons
    ~/  %make-omicrons
    |=  tables=(list table-dat)
    ^-  [bpoly fpoly]
    =/  os
      %+  turn  tables
      |=  [t=table-mary *]
      ~(omicron quot t)
    [(init-bpoly os) (init-fpoly (turn os lift))]
  ::
  ++  make-composition-poly
    ~/  %make-composition-poly
    |=  $:  =proof
            omicrons-bpoly=bpoly
            heights=(list @)
            tworow-trace-polys-eval=(list bpoly)
            constraint-map=(map @ constraints)
            count-map=(map @ constraint-counts)
            augmented-chals=bpoly
            dyn-list=(list bpoly)
            is-extra=?
        ==
    ^-  bpoly
    =/  num-tables=@  (lent heights)
    =/  constraint-counts=(list @)
      %+  turn  (range num-tables)
      |=  i=@
      =/  cs  (~(got by count-map) i)
      =/  c
        ;:  add
            boundary.cs
            row.cs
            transition.cs
            terminal.cs
        ==
      =?  c  is-extra  (add c extra.cs)
      c
    =/  composition-weights
      =/  num-constraints=@  (sum constraint-counts)
      =/  comp-weights
        =/  rng  ~(prover-fiat-shamir proof-stream proof)
        =^  belt-list  rng  (belts:rng (mul 2 num-constraints))
        (init-bpoly belt-list)
      %-  ~(gas by *(map @ bpoly))
      =-  -<
      %+  roll  (range num-tables)
      |=  [i=@ acc=(list [@ bpoly]) num=@]
      =/  num-constraints  (snag i constraint-counts)
      :_  (add num (mul 2 num-constraints))
      [[i (~(swag bop comp-weights) num (mul 2 num-constraints))] acc]
    %-  compute-composition-poly
    :*  omicrons-bpoly
        heights
        tworow-trace-polys-eval
        constraint-map
        count-map
        composition-weights
        augmented-chals
        dyn-list
        is-extra
    ==
  ::
  ++  make-trace-evals
    ~/  %make-trace-evals
    |=  [tworow-trace-polys=(list mary) eval-point=felt]
    ^-  fpoly
    %-  init-fpoly
    %-  zing
    %+  turn  tworow-trace-polys
    |=  polys=mary
    %+  turn  (range len.array.polys)
    |=  i=@
    =/  b=bpoly  (~(snag-as-bpoly ave polys) i)
    (bpeval-lift b eval-point)
  ::
  ++  add-commitments
    ~/  %add-commitments
    |=  [=proof fri-indices=(list @) commitments=(list codeword-commitments)]
    ^+  proof
    %+  roll  fri-indices
    |=  [idx=@ proof=_proof]
    |-
    ?~  commitments
      proof
    =,  i.commitments
    =/  elem  (~(change-step ave (~(snag-as-mary ave codewords) idx)) 1)
    =/  axis  (index-to-axis:merkle p.merk-heap idx)
    =/  opening  (build-merk-proof:merkle q.merk-heap axis)
    $(commitments t.commitments, proof (~(push proof-stream proof) m-pathbf+[(tail elem) path.opening]))
  ::
  ++  weld-terminals
    ~/  %weld-terminals
    |=  dyn-list=(list bpoly)
    ^-  bpoly
    %+  roll  dyn-list
    |=  [p=bpoly acc=bpoly]
    (~(weld bop acc) p)
  ::
  ++  make-second-row-trace-polys
    ~/  %make-second-row-trace-polys
    |=  tables=(list table-dat)
    ^-  (list mary)
    %+  turn  tables
    |=  t=table-dat
    %-  zing-bpolys
    =/  polys  (transpose-bpolys p.p.t)
    %+  turn  (range len.array.polys)
    |=  i=@
    =/  bp=bpoly  (~(snag-as-bpoly ave polys) i)
    (bp-ifft (bp-shift-by-unity bp 1))
  ::
  ++  make-trace-polys
    ~/  %make-trace-polys
    |=  [base=(list mary) ext=(list mary) mega-ext=(list mary)]
    ^-  (list mary)
    %+  turn  (zip-up base (zip-up ext mega-ext))
    |=  [bm=mary em=mary mem=mary]
    ^-  mary
    (~(weld ave bm) (~(weld ave em) mem))
  ::
  ++  make-composition-codewords
    ~/  %make-composition-codewords
    |=  [composition-pieces=(list bpoly) fri-domain-len=@]
    ^-  mary
    %-  zing-bpolys
    %+  turn  composition-pieces
    |=(=bpoly (bp-coseword bpoly g fri-domain-len))
  ::
  ++  make-deep-challenge
    ~/  %make-deep-challenge
    |=  [=proof n=@]
    ^-  felt
    =/  rng  ~(prover-fiat-shamir proof-stream proof)
    =^  deep-candidate  rng  $:felt:rng
    =/  exp-offset  (lift (bpow generator:stark-engine n))
    |-
    =/  exp-deep-can  (fpow deep-candidate n)
    ?.  ?|(=(exp-deep-can f1) =(exp-deep-can exp-offset))
      deep-candidate
    =^  felt  rng  $:felt:rng
    $(deep-candidate felt)
  ::
  ++  make-composition-piece-evals
    ~/  %make-composition-piece-evals
    |=  [deep-challenge=felt composition-pieces=(list fpoly)]
    ^-  fpoly
    =/  c  (fpow deep-challenge (lent composition-pieces))
    %-  init-fpoly
    %+  turn  composition-pieces
    |=(=fpoly (fpeval fpoly c))
  --
--
