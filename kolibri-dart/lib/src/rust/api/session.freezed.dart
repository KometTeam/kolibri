// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'session.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;

/// @nodoc
mixin _$UploadEvent {
  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType && other is UploadEvent);
  }

  @override
  int get hashCode => runtimeType.hashCode;

  @override
  String toString() {
    return 'UploadEvent()';
  }
}

/// @nodoc
class $UploadEventCopyWith<$Res> {
  $UploadEventCopyWith(UploadEvent _, $Res Function(UploadEvent) __);
}

/// Adds pattern-matching-related methods to [UploadEvent].
extension UploadEventPatterns on UploadEvent {
  /// A variant of `map` that fallback to returning `orElse`.
  ///
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case final Subclass value:
  ///     return ...;
  ///   case _:
  ///     return orElse();
  /// }
  /// ```

  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(UploadEvent_Progress value)? progress,
    TResult Function(UploadEvent_Done value)? done,
    TResult Function(UploadEvent_Error value)? error,
    required TResult orElse(),
  }) {
    final _that = this;
    switch (_that) {
      case UploadEvent_Progress() when progress != null:
        return progress(_that);
      case UploadEvent_Done() when done != null:
        return done(_that);
      case UploadEvent_Error() when error != null:
        return error(_that);
      case _:
        return orElse();
    }
  }

  /// A `switch`-like method, using callbacks.
  ///
  /// Callbacks receives the raw object, upcasted.
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case final Subclass value:
  ///     return ...;
  ///   case final Subclass2 value:
  ///     return ...;
  /// }
  /// ```

  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(UploadEvent_Progress value) progress,
    required TResult Function(UploadEvent_Done value) done,
    required TResult Function(UploadEvent_Error value) error,
  }) {
    final _that = this;
    switch (_that) {
      case UploadEvent_Progress():
        return progress(_that);
      case UploadEvent_Done():
        return done(_that);
      case UploadEvent_Error():
        return error(_that);
    }
  }

  /// A variant of `map` that fallback to returning `null`.
  ///
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case final Subclass value:
  ///     return ...;
  ///   case _:
  ///     return null;
  /// }
  /// ```

  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(UploadEvent_Progress value)? progress,
    TResult? Function(UploadEvent_Done value)? done,
    TResult? Function(UploadEvent_Error value)? error,
  }) {
    final _that = this;
    switch (_that) {
      case UploadEvent_Progress() when progress != null:
        return progress(_that);
      case UploadEvent_Done() when done != null:
        return done(_that);
      case UploadEvent_Error() when error != null:
        return error(_that);
      case _:
        return null;
    }
  }

  /// A variant of `when` that fallback to an `orElse` callback.
  ///
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case Subclass(:final field):
  ///     return ...;
  ///   case _:
  ///     return orElse();
  /// }
  /// ```

  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(BigInt sent, BigInt total)? progress,
    TResult Function(int status, Uint8List body)? done,
    TResult Function(String message)? error,
    required TResult orElse(),
  }) {
    final _that = this;
    switch (_that) {
      case UploadEvent_Progress() when progress != null:
        return progress(_that.sent, _that.total);
      case UploadEvent_Done() when done != null:
        return done(_that.status, _that.body);
      case UploadEvent_Error() when error != null:
        return error(_that.message);
      case _:
        return orElse();
    }
  }

  /// A `switch`-like method, using callbacks.
  ///
  /// As opposed to `map`, this offers destructuring.
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case Subclass(:final field):
  ///     return ...;
  ///   case Subclass2(:final field2):
  ///     return ...;
  /// }
  /// ```

  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(BigInt sent, BigInt total) progress,
    required TResult Function(int status, Uint8List body) done,
    required TResult Function(String message) error,
  }) {
    final _that = this;
    switch (_that) {
      case UploadEvent_Progress():
        return progress(_that.sent, _that.total);
      case UploadEvent_Done():
        return done(_that.status, _that.body);
      case UploadEvent_Error():
        return error(_that.message);
    }
  }

  /// A variant of `when` that fallback to returning `null`
  ///
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case Subclass(:final field):
  ///     return ...;
  ///   case _:
  ///     return null;
  /// }
  /// ```

  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(BigInt sent, BigInt total)? progress,
    TResult? Function(int status, Uint8List body)? done,
    TResult? Function(String message)? error,
  }) {
    final _that = this;
    switch (_that) {
      case UploadEvent_Progress() when progress != null:
        return progress(_that.sent, _that.total);
      case UploadEvent_Done() when done != null:
        return done(_that.status, _that.body);
      case UploadEvent_Error() when error != null:
        return error(_that.message);
      case _:
        return null;
    }
  }
}

/// @nodoc

class UploadEvent_Progress extends UploadEvent {
  const UploadEvent_Progress({required this.sent, required this.total})
      : super._();

  final BigInt sent;
  final BigInt total;

  /// Create a copy of UploadEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $UploadEvent_ProgressCopyWith<UploadEvent_Progress> get copyWith =>
      _$UploadEvent_ProgressCopyWithImpl<UploadEvent_Progress>(
          this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is UploadEvent_Progress &&
            (identical(other.sent, sent) || other.sent == sent) &&
            (identical(other.total, total) || other.total == total));
  }

  @override
  int get hashCode => Object.hash(runtimeType, sent, total);

  @override
  String toString() {
    return 'UploadEvent.progress(sent: $sent, total: $total)';
  }
}

/// @nodoc
abstract mixin class $UploadEvent_ProgressCopyWith<$Res>
    implements $UploadEventCopyWith<$Res> {
  factory $UploadEvent_ProgressCopyWith(UploadEvent_Progress value,
          $Res Function(UploadEvent_Progress) _then) =
      _$UploadEvent_ProgressCopyWithImpl;
  @useResult
  $Res call({BigInt sent, BigInt total});
}

/// @nodoc
class _$UploadEvent_ProgressCopyWithImpl<$Res>
    implements $UploadEvent_ProgressCopyWith<$Res> {
  _$UploadEvent_ProgressCopyWithImpl(this._self, this._then);

  final UploadEvent_Progress _self;
  final $Res Function(UploadEvent_Progress) _then;

  /// Create a copy of UploadEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  $Res call({
    Object? sent = null,
    Object? total = null,
  }) {
    return _then(UploadEvent_Progress(
      sent: null == sent
          ? _self.sent
          : sent // ignore: cast_nullable_to_non_nullable
              as BigInt,
      total: null == total
          ? _self.total
          : total // ignore: cast_nullable_to_non_nullable
              as BigInt,
    ));
  }
}

/// @nodoc

class UploadEvent_Done extends UploadEvent {
  const UploadEvent_Done({required this.status, required this.body})
      : super._();

  final int status;
  final Uint8List body;

  /// Create a copy of UploadEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $UploadEvent_DoneCopyWith<UploadEvent_Done> get copyWith =>
      _$UploadEvent_DoneCopyWithImpl<UploadEvent_Done>(this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is UploadEvent_Done &&
            (identical(other.status, status) || other.status == status) &&
            const DeepCollectionEquality().equals(other.body, body));
  }

  @override
  int get hashCode => Object.hash(
      runtimeType, status, const DeepCollectionEquality().hash(body));

  @override
  String toString() {
    return 'UploadEvent.done(status: $status, body: $body)';
  }
}

/// @nodoc
abstract mixin class $UploadEvent_DoneCopyWith<$Res>
    implements $UploadEventCopyWith<$Res> {
  factory $UploadEvent_DoneCopyWith(
          UploadEvent_Done value, $Res Function(UploadEvent_Done) _then) =
      _$UploadEvent_DoneCopyWithImpl;
  @useResult
  $Res call({int status, Uint8List body});
}

/// @nodoc
class _$UploadEvent_DoneCopyWithImpl<$Res>
    implements $UploadEvent_DoneCopyWith<$Res> {
  _$UploadEvent_DoneCopyWithImpl(this._self, this._then);

  final UploadEvent_Done _self;
  final $Res Function(UploadEvent_Done) _then;

  /// Create a copy of UploadEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  $Res call({
    Object? status = null,
    Object? body = null,
  }) {
    return _then(UploadEvent_Done(
      status: null == status
          ? _self.status
          : status // ignore: cast_nullable_to_non_nullable
              as int,
      body: null == body
          ? _self.body
          : body // ignore: cast_nullable_to_non_nullable
              as Uint8List,
    ));
  }
}

/// @nodoc

class UploadEvent_Error extends UploadEvent {
  const UploadEvent_Error({required this.message}) : super._();

  final String message;

  /// Create a copy of UploadEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $UploadEvent_ErrorCopyWith<UploadEvent_Error> get copyWith =>
      _$UploadEvent_ErrorCopyWithImpl<UploadEvent_Error>(this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is UploadEvent_Error &&
            (identical(other.message, message) || other.message == message));
  }

  @override
  int get hashCode => Object.hash(runtimeType, message);

  @override
  String toString() {
    return 'UploadEvent.error(message: $message)';
  }
}

/// @nodoc
abstract mixin class $UploadEvent_ErrorCopyWith<$Res>
    implements $UploadEventCopyWith<$Res> {
  factory $UploadEvent_ErrorCopyWith(
          UploadEvent_Error value, $Res Function(UploadEvent_Error) _then) =
      _$UploadEvent_ErrorCopyWithImpl;
  @useResult
  $Res call({String message});
}

/// @nodoc
class _$UploadEvent_ErrorCopyWithImpl<$Res>
    implements $UploadEvent_ErrorCopyWith<$Res> {
  _$UploadEvent_ErrorCopyWithImpl(this._self, this._then);

  final UploadEvent_Error _self;
  final $Res Function(UploadEvent_Error) _then;

  /// Create a copy of UploadEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  $Res call({
    Object? message = null,
  }) {
    return _then(UploadEvent_Error(
      message: null == message
          ? _self.message
          : message // ignore: cast_nullable_to_non_nullable
              as String,
    ));
  }
}

// dart format on
