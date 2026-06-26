import { Injectable, NotFoundException, BadRequestException } from '@nestjs/common';
import { InjectRepository } from '@nestjs/typeorm';
import { Repository } from 'typeorm';
import { BoardStyle } from './entities/board-style.entity';
import { CreateBoardStyleDto } from './dto/create-board-style.dto';
import { UpdateBoardStyleDto } from './dto/update-board-style.dto';
import { BoardStylesPaginationDto } from './dto/board-styles-pagination.dto';
import {
  PaginationService,
  PaginatedResponse,
  SortOrder,
} from '../../common';
import { RedisService } from '../redis/redis.service';

const BOARD_STYLE_SORT_FIELDS = [
  'created_at',
  'updated_at',
  'name',
  'price',
  'id',
] as const;

@Injectable()
export class BoardStylesService {
  constructor(
    @InjectRepository(BoardStyle)
    private readonly boardStyleRepository: Repository<BoardStyle>,
    private readonly paginationService: PaginationService,
    private readonly redisService: RedisService,
  ) {}

  async create(createBoardStyleDto: CreateBoardStyleDto): Promise<BoardStyle> {
    const trimmedName = createBoardStyleDto.name?.trim();
    if (!trimmedName) {
      throw new BadRequestException('board style name is required');
    }

    const style = this.boardStyleRepository.create({
      ...createBoardStyleDto,
      name: trimmedName,
    });
    const saved = await this.boardStyleRepository.save(style);
    await this.invalidateCache();
    return saved;
  }

  async findAll(
    paginationDto: BoardStylesPaginationDto,
  ): Promise<PaginatedResponse<BoardStyle>> {
    const qb = this.boardStyleRepository.createQueryBuilder('board_style');

    if (paginationDto.is_premium !== undefined) {
      qb.andWhere('board_style.is_premium = :isPremium', {
        isPremium: paginationDto.is_premium,
      });
    }

    const pagination = {
      ...paginationDto,
      sortBy: paginationDto.sortBy ?? 'created_at',
      sortOrder: paginationDto.sortOrder ?? SortOrder.DESC,
    };

    return this.paginationService.paginate(
      qb,
      pagination,
      ['name', 'description'],
      [...BOARD_STYLE_SORT_FIELDS],
    );
  }

  async findOne(id: number): Promise<BoardStyle> {
    const style = await this.boardStyleRepository.findOne({ where: { id } });
    if (!style) {
      throw new NotFoundException(`Board style with ID ${id} not found`);
    }
    return style;
  }

  async update(
    id: number,
    updateBoardStyleDto: UpdateBoardStyleDto,
  ): Promise<BoardStyle> {
    const style = await this.findOne(id);
    const updatedStyle = this.boardStyleRepository.merge(
      style,
      updateBoardStyleDto,
    );
    const saved = await this.boardStyleRepository.save(updatedStyle);
    await this.invalidateCache(id);
    return saved;
  }

  async remove(id: number): Promise<void> {
    const style = await this.findOne(id);
    await this.boardStyleRepository.remove(style);
    await this.invalidateCache(id);
  }

  private async invalidateCache(id?: number) {
    await this.redisService.delByPattern('tycoon:board-styles:board-styles:*');
    if (id) {
      await this.redisService.delByPattern(
        `tycoon:board-styles:board-styles:${id}:*`,
      );
    }
  }
}
