; ModuleID = 'autocfg_7ab36f624092f1a3_1.7753f0b441ded408-cgu.0'
source_filename = "autocfg_7ab36f624092f1a3_1.7753f0b441ded408-cgu.0"
target datalayout = "e-m:e-p:32:32-Fi8-i64:64-v128:64:128-a:0:32-n32-S64"
target triple = "armv7-unknown-linux-gnueabihf"

@alloc_f93507f8ba4b5780b14b2c2584609be0 = private unnamed_addr constant [8 x i8] c"\00\00\00\00\00\00\F0?", align 8
@alloc_ef0a1f828f3393ef691f2705e817091c = private unnamed_addr constant [8 x i8] c"\00\00\00\00\00\00\00@", align 8

; autocfg_7ab36f624092f1a3_1::probe
; Function Attrs: uwtable
define void @_ZN26autocfg_7ab36f624092f1a3_15probe17hde7439373eeab8e9E() unnamed_addr #0 {
start:
; call core::f64::<impl f64>::total_cmp
  %_1 = call i8 @"_ZN4core3f6421_$LT$impl$u20$f64$GT$9total_cmp17h8f346c9d08fdf754E"(ptr align 8 @alloc_f93507f8ba4b5780b14b2c2584609be0, ptr align 8 @alloc_ef0a1f828f3393ef691f2705e817091c) #3
  ret void
}

; core::f64::<impl f64>::total_cmp
; Function Attrs: inlinehint uwtable
define internal i8 @"_ZN4core3f6421_$LT$impl$u20$f64$GT$9total_cmp17h8f346c9d08fdf754E"(ptr align 8 %self, ptr align 8 %other) unnamed_addr #1 {
start:
  %_6 = alloca [8 x i8], align 8
  %_3 = alloca [8 x i8], align 8
  %_5 = load double, ptr %self, align 8
  %_4 = bitcast double %_5 to i64
  store i64 %_4, ptr %_3, align 8
  %_8 = load double, ptr %other, align 8
  %_7 = bitcast double %_8 to i64
  store i64 %_7, ptr %_6, align 8
  %_13 = load i64, ptr %_3, align 8
  %_12 = ashr i64 %_13, 63
  %_10 = lshr i64 %_12, 1
  %0 = load i64, ptr %_3, align 8
  %1 = xor i64 %0, %_10
  store i64 %1, ptr %_3, align 8
  %_18 = load i64, ptr %_6, align 8
  %_17 = ashr i64 %_18, 63
  %_15 = lshr i64 %_17, 1
  %2 = load i64, ptr %_6, align 8
  %3 = xor i64 %2, %_15
  store i64 %3, ptr %_6, align 8
  %4 = load i64, ptr %_3, align 8
  %5 = load i64, ptr %_6, align 8
  %_0 = call i8 @llvm.scmp.i8.i64(i64 %4, i64 %5)
  ret i8 %_0
}

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare range(i8 -1, 2) i8 @llvm.scmp.i8.i64(i64, i64) #2

attributes #0 = { uwtable "target-cpu"="generic" "target-features"="+thumb2,+v5te,+v6,+v6k,+v6t2,+v7,+d32,+vfp2,+vfp3,-aes,-d32,-dotprod,-fp-armv8,-fullfp16,-i8mm,-neon,-sha2,-vfp3,-vfp4,+thumb2,-aes,-dotprod,-fullfp16,-i8mm,-neon,-sha2" }
attributes #1 = { inlinehint uwtable "target-cpu"="generic" "target-features"="+thumb2,+v5te,+v6,+v6k,+v6t2,+v7,+d32,+vfp2,+vfp3,-aes,-d32,-dotprod,-fp-armv8,-fullfp16,-i8mm,-neon,-sha2,-vfp3,-vfp4,+thumb2,-aes,-dotprod,-fullfp16,-i8mm,-neon,-sha2" }
attributes #2 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #3 = { inlinehint }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{!"rustc version 1.93.0 (254b59607 2026-01-19)"}
