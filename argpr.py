"""Minimal typed argument parser.

```python
@command("cluster")                  # or bare `@command()`; in this case,
def cluster(                         # the command name becomes the function name
    run_dir: Path,                   # whose `_` is replaced by `-`.
    near_dup_cosine: float = 0.85,
    preds: list[Path] = [],
    artifacts_dir: Path | None = None,
) -> None: ...

cluster(run_dir=Path("x"))                            # a plain call
cluster.run(["--run-dir", "x", "--preds", "a", "--preds", "b"])
cluster.flags                                         # {"--run-dir", ...}

if __name__ == "__main__":
    init(__name__)   # python -m pkg.module cluster --run-dir x --preds a
```

Every parameter is a `--kebab-case` option taking one value,
spelled `--flag value` or `--flag=value`.
A positional-only parameter (one declared before `/`) is a positional argument instead.

For `list`-typed parameters, repeat the flag (like `preds` above).
For a `bool` parameter, a bare `--flag` is true, and
you can also explicitly set like `--flag=false` (or `=true`, `=1`, `=0`, `=yes`, `=no`).

Related: https://github.com/swansonk14/typed-argument-parser
"""

import argparse
import functools
import inspect
import sys
import types
from collections.abc import Callable, Iterator, Mapping, Sequence
from dataclasses import dataclass
from datetime import date, datetime
from enum import Enum, unique
from pathlib import Path as _Path
from typing import (
    Any,
    NoReturn,
    Union,
    cast,
    get_args,
    get_origin,
    get_type_hints,
    overload,
)

from typing_extensions import override

SupportedScalarType = bool | int | float | str | date | datetime | _Path


def _parse_path(text: str) -> _Path:
    if not text:
        raise ValueError("empty path")
    return _Path(text)


def _parse_bool(text: str) -> bool:
    match text.lower().strip():
        case "true":
            return True
        case "false":
            return False
        case "1":
            return True
        case "0":
            return False
        case "yes":
            return True
        case "no":
            return False
        case _:
            raise ValueError(f"not a boolean: {text!r}")


class ArgumentError(ValueError):
    pass


@unique
class Type(Enum):
    Bool = bool
    Int = int
    Float = float
    Str = str
    Date = date
    Datetime = datetime
    Path = _Path
    ##
    StrList = list[str]
    IntList = list[int]
    FloatList = list[float]
    DateList = list[date]
    DatetimeList = list[datetime]
    PathList = list[_Path]

    @property
    def is_list(self) -> bool:
        return self in (
            Type.StrList,
            Type.IntList,
            Type.FloatList,
            Type.DateList,
            Type.DatetimeList,
            Type.PathList,
        )

    @property
    def scalar_type(self) -> type[SupportedScalarType]:
        match self:
            case Type.StrList:
                return str
            case Type.IntList:
                return int
            case Type.FloatList:
                return float
            case Type.DateList:
                return date
            case Type.DatetimeList:
                return datetime
            case Type.PathList:
                return _Path
            case _:
                return self.value

    def parse_item(self, text: str) -> SupportedScalarType:

        st = self.scalar_type

        if st is bool:
            parser = _parse_bool
        elif st is int:
            parser = int
        elif st is float:
            parser = float
        elif st is str:
            parser = str
        elif st is date:
            parser = date.fromisoformat
        elif st is datetime:
            parser = datetime.fromisoformat
        elif st is _Path:
            parser = _parse_path
        else:
            raise ArgumentError(f"Unsupported type: `{st}`")

        return cast(Callable[[str], Any], parser)(text)

    def accepts(self, value: Any) -> bool:
        """Check if a Python `value` fits this `Type`."""

        def _accepts_item(type_: Type, value: SupportedScalarType) -> bool:
            if isinstance(value, bool):
                # a `bool` is an `int`, so allow `Bool` only here.
                return type_.scalar_type is bool

            if type_.scalar_type is float:
                return isinstance(value, (int, float))

            return isinstance(value, type_.scalar_type)

        if self.is_list:
            return isinstance(value, list) and all(_accepts_item(self, v) for v in value)

        return _accepts_item(self, value)




class _Parser(argparse.ArgumentParser):
    @override
    def error(self, message: str) -> NoReturn:
        raise ArgumentError(message)


class _Append(argparse.Action):
    def __call__(
        self,
        parser: argparse.ArgumentParser,
        namespace: argparse.Namespace,
        values: Any,
        option_string: str | None = None,
    ) -> None:
        current = getattr(namespace, self.dest, None)
        fresh = current is None or current is self.default
        items = list[Any]() if fresh else list(current)
        items.append(values)
        setattr(namespace, self.dest, items)


@dataclass(frozen=True)
class Parameter:
    name: str
    type: Type
    optional: bool
    positional: bool = False
    default: Any = inspect.Parameter.empty

    @property
    def flag(self) -> str:
        return "--" + self.name.replace("_", "-")

    @property
    def spelling(self) -> str:
        return self.name if self.positional else self.flag

    @property
    def required(self) -> bool:
        return self.default is inspect.Parameter.empty

    @property
    def type_name(self) -> str:
        return self.type.name + (" | None" if self.optional else "")

    def convert(self, text: str) -> Any:
        """Parse one value."""
        try:
            return self.type.parse_item(text)
        except ValueError:
            raise argparse.ArgumentTypeError(
                f"{text!r} is not a valid {self.type.scalar_type.__name__}"
            ) from None

    def add_to(self, parser: argparse.ArgumentParser) -> None:
        """Declare the parameter on an `argparse` parser."""
        options: dict[str, Any] = {"type": self.convert}
        if not self.required:
            options["default"] = self.default

        if self.positional:
            if self.type.is_list:
                options["nargs"] = "+" if self.required else "*"
            elif not self.required:
                options["nargs"] = "?"
            parser.add_argument(self.name, **options)
            return

        options.update(
            dest=self.name,
            required=self.required,
            metavar=self.type.scalar_type.__name__,
        )
        if self.type.is_list:
            options["action"] = _Append
        elif self.type is Type.Bool:
            options.update(nargs="?", const=True)  # a bare flag is true
        parser.add_argument(self.flag, **options)


class PrettyPrint:
    @classmethod
    def type_repr(cls, annotation: Any) -> str:
        origin = get_origin(annotation)

        if origin in (Union, types.UnionType):
            return " | ".join(cls.type_repr(member) for member in get_args(annotation))

        if origin is not None:
            args = ", ".join(cls.type_repr(arg) for arg in get_args(annotation))
            return f"{origin.__name__}[{args}]"

        return getattr(annotation, "__name__", repr(annotation))

    @classmethod
    def display(cls, value: Any) -> str:
        if isinstance(value, str):
            return repr(value)
        if isinstance(value, list):
            return "[" + ", ".join(cls.display(item) for item in value) + "]"
        return str(value)

    @classmethod
    def table(cls, rows: Sequence[Sequence[str]], indent: str = "  ") -> str:
        widths = [max(len(row[i]) for row in rows) for i in range(len(rows[0]))]
        return "\n".join(
            (indent + "  ".join(cell.ljust(w) for cell, w in zip(row, widths))).rstrip()
            for row in rows
        )


def _unwrap_optional(annotation: Any) -> tuple[Any, bool]:
    """Split `T | None` (or `Optional[T]`) into `(T, True)`;
    else `(annotation, False)`.
    """

    if get_origin(annotation) in (Union, types.UnionType):
        members = get_args(annotation)
        inner = [member for member in members if member is not type(None)]

        if len(members) == 2 and len(inner) == 1:
            return inner[0], True

    return annotation, False


def _parameters(function: Callable[..., Any]) -> dict[str, Parameter]:
    """Check `function`'s signature and describe each parameter.

    `TypeError` for:
    - `*args`/`**kwargs`
    - missing or unsupported annotation
    - a default that does not fit its type
    """

    where = f"{function.__module__}.{function.__qualname__}"
    try:
        hints = get_type_hints(function)
    except NameError as error:
        raise TypeError(f"{where}: cannot resolve annotations: {error}") from None

    parameters: dict[str, Parameter] = {}
    for name, param in inspect.signature(function).parameters.items():
        if param.kind in (
            inspect.Parameter.VAR_POSITIONAL,
            inspect.Parameter.VAR_KEYWORD,
        ):
            raise TypeError(
                f"{where}: parameter {name!r} is {param.kind.description}; "
                "every parameter must take exactly one value"
            )

        if name not in hints:
            raise TypeError(f"{where}: parameter {name!r} has no type annotation")

        annotation, optional = _unwrap_optional(hints[name])

        try:
            type_ = Type(annotation)
        except ValueError:
            supported = ", ".join(
                PrettyPrint.type_repr(member.value) for member in Type
            )
            raise TypeError(
                f"{where}: parameter {name!r} is annotated "
                f"{PrettyPrint.type_repr(hints[name])}; supported: {supported} (each also | None)"
            ) from None

        default = param.default
        if default is not inspect.Parameter.empty and not (
            (optional and default is None) or type_.accepts(default)
        ):
            raise TypeError(
                f"{where}: default {default!r} of {name!r} is not {type_.name}"
            )

        parameters[name] = Parameter(
            name,
            type_,
            optional,
            positional=param.kind is inspect.Parameter.POSITIONAL_ONLY,
            default=default,
        )

    return parameters


class Command:
    def __init__(self, function: Callable[..., Any], name: str | None = None) -> None:
        self.function = function
        self.name = name or function.__name__.lower().replace("_", "-")
        self.parameters = _parameters(function)
        functools.update_wrapper(self, function)

    @property
    def flags(self) -> frozenset[str]:
        """Every `--flag` for the command."""
        return frozenset(p.flag for p in self.parameters.values() if not p.positional)

    def __call__(self, *args: Any, **kwargs: Any) -> Any:
        return self.function(*args, **kwargs)

    def __repr__(self) -> str:
        return (
            f"Command({self.name!r}, "
            f"{self.function.__module__}:{self.function.__qualname__})"
        )

    def __str__(self) -> str:
        return self.usage()

    def configure(self, parser: argparse.ArgumentParser) -> None:
        """Declare every parameter on an `argparse` parser."""
        for parameter in self.parameters.values():
            parameter.add_to(parser)

    def parse(self, argv: Sequence[str]) -> dict[str, Any]:
        parser = _Parser(prog=self.name, add_help=False)
        self.configure(parser)
        return vars(parser.parse_args(list(argv)))

    def invoke(self, values: Mapping[str, Any]) -> Any:
        """Call the function with parsed `values` (`parse`'s result)."""
        values = dict(values)
        positional = [
            values.pop(p.name) for p in self.parameters.values() if p.positional
        ]
        return self(*positional, **values)

    def run(self, argv: Sequence[str]) -> Any:
        """Parse ``argv`` and call the function with the result."""
        return self.invoke(self.parse(argv))

    def usage(self) -> str:
        """The command's name and, per line, each argument with its type and default."""
        rows = [
            (
                p.spelling,
                p.type_name,
                "required"
                if p.required
                else f"default {PrettyPrint.display(p.default)}",
            )
            for p in self.parameters.values()
        ]
        return (
            self.name + "\n" + (PrettyPrint.table(rows) if rows else "  (no arguments)")
        )

    def format_arguments(self, values: Mapping[str, Any]) -> str:
        """Pretty-print bound values (e.g. ``parse``'s result), one per line."""
        rows = [
            (name, f"= {PrettyPrint.display(values[name])}")
            for name in self.parameters
            if name in values
        ]
        return (
            self.name + "\n" + (PrettyPrint.table(rows) if rows else "  (no arguments)")
        )


@overload
def command(name: None = None) -> Callable[[Callable[..., Any]], Command]: ...


@overload
def command(name: str) -> Callable[[Callable[..., Any]], Command]: ...


@overload
def command(name: Callable[..., Any]) -> Command: ...


def command(
    name: str | Callable[..., Any] | None = None,
) -> Command | Callable[[Callable[..., Any]], Command]:
    """Turn a function into a `Command`: `@command`, `@command()`, `@command("name")`."""
    if callable(name):
        return Command(name)
    return lambda function: Command(function, name)


def _module_commands(module: types.ModuleType) -> Iterator[Command]:
    """The commands defined (not merely imported) in ``module``, in definition order."""
    for value in vars(module).values():
        if isinstance(value, Command) and value.__module__ == module.__name__:
            yield value


def init(name: str) -> Any:
    """Run the command for the module `name`.

    Call it as `init(__name__)` under `if __name__ == "__main__":`, and
    the module runs as `python -m pkg.module <command> --flag value ...`.
    """

    module = sys.modules[name]
    spec = module.__spec__
    prog = None if spec is None else "python -m " + spec.name.removesuffix(".__main__")
    parser = argparse.ArgumentParser(prog=prog)
    subparsers = parser.add_subparsers(dest="command", required=True)

    commands: dict[str, Command] = {}
    for command_ in _module_commands(module):
        if command_.name in commands:
            raise ValueError(f"{name}: duplicate command name {command_.name!r}")
        commands[command_.name] = command_
        command_.configure(subparsers.add_parser(command_.name))

    values = vars(parser.parse_args())
    selected = commands[values.pop("command")]
    try:
        return selected.invoke(values)
    except ArgumentError as error:
        subparsers.choices[selected.name].error(str(error))
